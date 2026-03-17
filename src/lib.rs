//! # zen
//!
//! A spaced repetition CLI for active recall using FSRS algorithm.

#![forbid(unsafe_code)]

pub mod card;
pub mod commands;
pub mod database;
pub mod storage;

pub use card::Card;
