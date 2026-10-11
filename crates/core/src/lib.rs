//! netfoo's core: what a share is, what the privileged helper can be asked to
//! do, and the config files it writes for each.
//!
//! Nothing here needs root or talks to the system bus, so it's shared by the
//! app (which runs as the user) and the helper (which runs as root), and all
//! of it can be tested without either.
//!
//! - [`names`]: validated names and paths. Every value that ends up in a
//!   config file goes through one of these types, so the helper never writes
//!   a newline or a `[` it didn't mean to.
//! - [`share`]: Samba shares and NFS exports, as netfoo models them.
//! - [`state`]: everything netfoo manages, saved as one JSON file. The config
//!   files are generated from it, never parsed back.
//! - [`request`]: what the app asks the helper to do.
//! - [`bus`]: the helper's D-Bus and polkit names.
//! - [`linux`]: rendering the Linux config files.
//! - [`system`]: what's installed on this machine (read without root).

pub mod bus;
pub mod linux;
pub mod names;
pub mod request;
pub mod share;
pub mod state;
pub mod system;

pub use names::{AbsolutePath, InvalidValue, NfsClient, ShareName, UserName};
pub use request::{Request, Secret};
pub use share::{NfsExport, SmbAccess, SmbShare, check_nfs_exports, check_smb_shares};
pub use state::State;
