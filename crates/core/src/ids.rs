//! Newtype wrappers over `Uuid` for every primary entity id in the schema.
//!
//! Newtypes prevent accidentally passing a `ProgramId` where a `CourseId` is
//! expected — a real risk given how many `Uuid` foreign keys this schema
//! carries (see `IMPLEMENTATION_PLAN.md` §3.1). Each id is `#[sqlx(transparent)]`
//! so it binds/reads directly against the corresponding `UUID` column in
//! `sqlx::query_as!`/`sqlx::query!`, and `#[serde(transparent)]` so it
//! (de)serialises as a bare UUID string on the wire, not `{"0": "..."}`.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
        #[sqlx(transparent)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            /// Generate a fresh random id (v4).
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Wrap an existing `Uuid` (e.g. parsed from a path param or a DB row).
            pub const fn from_uuid(id: Uuid) -> Self {
                Self(id)
            }

            /// Unwrap to the underlying `Uuid`.
            pub const fn into_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(Uuid::parse_str(s)?))
            }
        }

        impl From<Uuid> for $name {
            fn from(id: Uuid) -> Self {
                Self(id)
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Uuid {
                id.0
            }
        }
    };
}

define_id!(UserId);
define_id!(StudentId);
define_id!(ProgramId);
define_id!(SemesterId);
define_id!(CourseId);
define_id!(BlockId);
define_id!(LscId);
define_id!(DocumentId);
/// A `learning_sessions` row — the classroom session. Kept distinct from an
/// `auth_sessions` row (refresh-token/device session), which has no newtype
/// of its own and is addressed as a bare `Uuid` alongside its opaque token.
define_id!(SessionId);
