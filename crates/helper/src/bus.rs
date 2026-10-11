//! The helper's D-Bus service: one method, `Apply`, and a `Version` property.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use netfoo_core::bus::{BUS_NAME, OBJECT_PATH};
use netfoo_core::linux::Paths;
use netfoo_core::{Owner, Request};
use zbus::fdo;
use zbus::message::Header;
use zbus::names::UniqueName;

use crate::services::SystemServices;
use crate::users::primary_group;
use crate::{apply, polkit};

/// How long the helper waits for another request before exiting. D-Bus starts
/// it again when it's next needed.
const IDLE_TIMEOUT: Duration = Duration::from_secs(120);

/// Serves requests on the system bus until the helper has been idle for
/// [`IDLE_TIMEOUT`].
pub fn serve_until_idle() -> zbus::Result<()> {
    let activity = Arc::new(Activity::new());
    let helper = Helper {
        paths: Paths::system(),
        activity: activity.clone(),
        applying: Mutex::new(()),
    };

    // Keeps serving (on zbus's own thread) for as long as it's in scope.
    let _connection = zbus::blocking::connection::Builder::system()?
        .name(BUS_NAME)?
        .serve_at(OBJECT_PATH, helper)?
        .build()?;

    while !activity.idle_for(IDLE_TIMEOUT) {
        std::thread::sleep(Duration::from_secs(5));
    }
    Ok(())
}

struct Helper {
    paths: Paths,
    activity: Arc<Activity>,
    /// Held while a request is applied, so two requests never write the same
    /// files at once.
    applying: Mutex<()>,
}

#[zbus::interface(name = "io.netfoo.Helper1")]
impl Helper {
    /// Applies `request`, a JSON-encoded `netfoo_core::Request`, if polkit
    /// allows the caller to.
    ///
    /// JSON rather than D-Bus types: the app and the helper share the Rust
    /// types, so they can't disagree about the shape, and the validation
    /// happens as it's decoded.
    async fn apply(
        &self,
        request: &str,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> fdo::Result<()> {
        let _busy = self.activity.begin();

        let request: Request =
            serde_json::from_str(request).map_err(|e| fdo::Error::InvalidArgs(e.to_string()))?;
        let caller = header
            .sender()
            .ok_or_else(|| fdo::Error::AccessDenied("The caller is unknown.".into()))?;
        polkit::authorize(connection, caller, request.action()).await?;
        let owner = caller_account(connection, caller).await?;

        let _applying = self
            .applying
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        apply::apply(&request, owner, &self.paths, &SystemServices)
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    /// The helper's version, so the app can tell it's talking to the one it was built with.
    #[zbus(property)]
    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }
}

/// The account behind a bus name, as D-Bus vouches for it: its user, and that
/// user's primary group.
async fn caller_account(
    connection: &zbus::Connection,
    caller: &UniqueName<'_>,
) -> fdo::Result<Owner> {
    let uid = fdo::DBusProxy::new(connection)
        .await?
        .get_connection_unix_user(caller.clone().into())
        .await?;
    let gid = primary_group(uid)
        .ok_or_else(|| fdo::Error::Failed(format!("User {uid} has no account here.")))?;
    Ok(Owner { uid, gid })
}

/// Tracks when the helper was last used, so it can exit when it isn't.
struct Activity {
    last_used: Mutex<Instant>,
    /// Requests in progress (one may be waiting on a password prompt).
    in_progress: AtomicUsize,
}

impl Activity {
    fn new() -> Self {
        Self {
            last_used: Mutex::new(Instant::now()),
            in_progress: AtomicUsize::new(0),
        }
    }

    fn begin(&self) -> Busy<'_> {
        self.in_progress.fetch_add(1, Ordering::SeqCst);
        Busy(self)
    }

    fn idle_for(&self, duration: Duration) -> bool {
        let last_used = *self
            .last_used
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.in_progress.load(Ordering::SeqCst) == 0 && last_used.elapsed() >= duration
    }
}

/// A request in progress; finishing it (dropping this) counts as use.
struct Busy<'a>(&'a Activity);

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        *self
            .0
            .last_used
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Instant::now();
        self.0.in_progress.fetch_sub(1, Ordering::SeqCst);
    }
}
