//! Looking up accounts in the system's user database (/etc/passwd, or
//! whatever else it's configured to use).

use std::ffi::CString;

/// Distributions number people's accounts from 1000; 65534 is "nobody".
const PEOPLE: std::ops::Range<u32> = 1000..65534;

/// `uid`'s primary group.
pub fn primary_group(uid: u32) -> Option<u32> {
    lookup(|entry, buffer, found| {
        // SAFETY: getpwuid_r writes only into `entry` and `buffer` (whose length
        // it's given), and sets `found` to `entry` on success or null if there's none.
        unsafe { libc::getpwuid_r(uid, entry, buffer.as_mut_ptr(), buffer.len(), found) }
    })
    .map(|entry| entry.pw_gid)
}

/// Whether `name` is a person's account, not root or a system account.
pub fn is_person(name: &str) -> bool {
    let Ok(name) = CString::new(name) else {
        return false;
    };
    lookup(|entry, buffer, found| {
        // SAFETY: as for getpwuid_r above; `name` is a valid C string.
        unsafe {
            libc::getpwnam_r(
                name.as_ptr(),
                entry,
                buffer.as_mut_ptr(),
                buffer.len(),
                found,
            )
        }
    })
    .is_some_and(|entry| PEOPLE.contains(&entry.pw_uid))
}

/// Runs a reentrant getpw*_r lookup with room for its strings. Only the
/// entry's numbers are used, which don't point into the buffer.
fn lookup(
    call: impl FnOnce(&mut libc::passwd, &mut [libc::c_char], *mut *mut libc::passwd) -> libc::c_int,
) -> Option<libc::passwd> {
    // SAFETY: passwd is plain data; all-zero is a valid (empty) value.
    let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
    let mut found: *mut libc::passwd = std::ptr::null_mut();
    let mut buffer = vec![0 as libc::c_char; 16 * 1024];
    let status = call(&mut entry, &mut buffer, &mut found);
    (status == 0 && !found.is_null()).then_some(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_accounts_are_not_people() {
        assert!(!is_person("root"));
        assert!(!is_person("nobody"));
        assert!(!is_person("no-such-user-here"));
        assert_eq!(primary_group(0), Some(0));
    }
}
