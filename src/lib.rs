//! # zen
//!
//! A spaced repetition CLI for active recall using FSRS algorithm.

#![forbid(unsafe_code)]

pub mod bert_score;
pub mod card;
pub mod card_creation_tui;
pub mod commands;
pub mod config;
pub mod database;
pub mod editor;
pub mod finder;
pub mod llm_evaluator;
pub mod review;
pub mod review_tui;
pub mod stats_tui;
pub mod storage;
pub mod tui;

pub use card::Card;
