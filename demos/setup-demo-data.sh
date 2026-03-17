#!/bin/bash
# Setup demo data for VHS recordings

# Remove existing zen data directory
rm -rf ~/.zen

# Create a few sample cards
./target/release/zen new "What is the capital of France?" <<EOF
Paris is the capital and most populous city of France.

EOF

./target/release/zen new "What is the primary use of Rust programming language?" <<EOF
Rust is primarily used for systems programming, focusing on safety, speed, and concurrency.

EOF

./target/release/zen new "What does FSRS stand for?" <<EOF
FSRS stands for Free Spaced Repetition Scheduler.

EOF

./target/release/zen new "What is the purpose of spaced repetition?" <<EOF
Spaced repetition is a learning technique that involves reviewing information at increasing intervals to improve long-term retention.

EOF

echo "Demo data created!"
