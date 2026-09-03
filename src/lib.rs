//! Core library for `refaudit`.
//!
//! The crate reads a long-format table containing results generated against
//! multiple reference genomes and measures how stable those results are when
//! compared with a user-selected baseline.

pub mod audit;
pub mod input;
pub mod metrics;
pub mod model;
pub mod render;
