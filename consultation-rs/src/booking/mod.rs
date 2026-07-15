//! Deprecated compatibility facade for Appointment Hold.
//!
//! `booking` is route-adapter vocabulary for the public/internal HTTP aliases.
//! Canonical module ownership lives in [`crate::appointment::hold`].

#[deprecated(note = "use crate::appointment::hold::bootstrap")]
pub use crate::appointment::hold::bootstrap;
#[deprecated(note = "use crate::appointment::hold::handler")]
pub use crate::appointment::hold::handler;
#[deprecated(note = "use crate::appointment::hold::model")]
pub use crate::appointment::hold::model;
#[deprecated(note = "use crate::appointment::hold::repo")]
pub use crate::appointment::hold::repo;
#[deprecated(note = "use crate::appointment::hold::router")]
pub use crate::appointment::hold::router;
#[deprecated(note = "use crate::appointment::hold::service")]
pub use crate::appointment::hold::service;
#[deprecated(note = "use crate::appointment::hold::state")]
pub use crate::appointment::hold::state;
