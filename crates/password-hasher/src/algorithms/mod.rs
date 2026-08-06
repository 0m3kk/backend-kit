#[cfg(feature = "argon2")]
pub mod argon2;
#[cfg(feature = "bcrypt")]
pub mod bcrypt;
#[cfg(feature = "noop")]
pub mod noop;
#[cfg(feature = "pbkdf2")]
pub mod pbkdf2;

#[cfg(feature = "argon2")]
pub use argon2::{Argon2Config, Argon2Hasher};

#[cfg(feature = "bcrypt")]
pub use bcrypt::{BcryptConfig, BcryptHasher};

#[cfg(feature = "noop")]
pub use noop::NoopHasher;

#[cfg(feature = "pbkdf2")]
pub use pbkdf2::{Pbkdf2Config, Pbkdf2Hasher};
