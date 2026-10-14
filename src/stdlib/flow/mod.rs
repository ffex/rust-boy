pub mod emittable;
pub mod flow_if;
pub mod loops;
pub mod operation;

// Re-export for convenience
pub use emittable::{Call, Emittable, boxed};
pub use flow_if::{ComparisonOp, If, IfA, IfCall, IfConst};
pub use loops::{For, SwitchA, While};
pub use operation::{InstrOps, Op};
