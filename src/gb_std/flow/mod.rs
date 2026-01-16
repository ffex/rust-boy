pub mod emittable;
pub mod flow_if;
pub mod operation;

// Re-export for convenience
pub use emittable::Emittable;
pub use flow_if::{ComparisonOp, If};
pub use operation::{InstrOps, Op};
