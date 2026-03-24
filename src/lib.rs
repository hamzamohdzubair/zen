//! # zen
//!
//! A topic-based spaced repetition CLI with LLM-powered reviews using FSRS algorithm.

#![forbid(unsafe_code)]

pub mod commands;
pub mod config;
pub mod database;
pub mod llm_evaluator;
pub mod stats_tui;
pub mod topic;
pub mod topic_review;
pub mod topic_review_tui;
