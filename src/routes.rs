mod health_check;
mod home;
mod login;
mod newsletter;
mod subscriptions;
mod subscriptions_confirmation;

pub use health_check::*;
pub use home::*;
pub use login::*;
pub use newsletter::*;
pub use subscriptions::*;
pub use subscriptions_confirmation::*;

// A package is a bundle of one or more crates that provides a set of functionality

// A crate can come in one of two forms: a binary crate or a library crate.
// A package can contain as many binary crates as you like, but at most only one library crate.
// A package must contain at least one crate, whether that’s a library or binary crate.
