# How to Check Logs for Leaderless Voting

## 1. Real-Time Log Viewing (During Benchmark)

For local execution, logs are written to the working directory (default: `~/mysticeti-working`).

### View logs in real-time:

```bash
# Watch node 0 logs
tail -f ~/mysticeti-working/node-0.log

# Watch all node logs
tail -f ~/mysticeti-working/node-*.log

# Watch client logs
tail -f ~/mysticeti-working/client.log
```

### View logs in separate terminals:

```bash
# Terminal 1 - Node 0
tail -f ~/mysticeti-working/node-0.log | grep -E "(get_pending_votes|Including.*vote|VoteRange)"

# Terminal 2 - Node 1
tail -f ~/mysticeti-working/node-1.log | grep -E "(get_pending_votes|Including.*vote|VoteRange)"

# Terminal 3 - All nodes (unfiltered)
tail -f ~/mysticeti-working/node-*.log
```

## 2. Enable Log Download (After Benchmark)

Edit `crates/orchestrator/assets/settings.yml`:

```yaml
# Enable log processing
log_processing: true

# Set logs directory
logs_dir: "./logs"
```

Then logs will be downloaded to: `./logs/logs-{commit}/logs-{parameters}/`

## 3. Enable Verbose Logging

Set the `RUST_LOG` environment variable before running:

```bash
# Debug level (shows our debug messages)
export RUST_LOG=debug

# Or more specific (only mysticeti-core)
export RUST_LOG=mysticeti_core=debug

# Run benchmark
cargo run --bin orchestrator -- benchmark --committee 4 --loads 200
```

## 4. What to Look For

### Success indicators (leaderless voting working):

1. **Vote creation messages:**
   ```
   get_pending_votes: pending=10, time_elapsed=..., should_create=true
   ```

2. **Votes being included in blocks:**
   ```
   Including 2 vote statements in block at round 5
   ```

3. **VoteRange statements in blocks:**
   Look for blocks containing `VoteRange` statements (not just `Share`)

### Check if votes are being created:

```bash
# Count vote creation messages
grep "get_pending_votes" ~/mysticeti-working/node-*.log | wc -l

# Count votes included in blocks
grep "Including.*vote statements" ~/mysticeti-working/node-*.log | wc -l
```

### Check transaction commitment:

```bash
# Look for transaction commitment messages
grep -i "committed\|certified" ~/mysticeti-working/node-*.log
```

## 5. Quick Check Script

Create a script to check logs:

```bash
#!/bin/bash
LOG_DIR=~/mysticeti-working

echo "=== Vote Creation Stats ==="
echo "Vote creation attempts:"
grep -c "get_pending_votes" $LOG_DIR/node-*.log 2>/dev/null || echo "0"

echo ""
echo "Votes included in blocks:"
grep -c "Including.*vote statements" $LOG_DIR/node-*.log 2>/dev/null || echo "0"

echo ""
echo "=== Recent Vote Activity ==="
grep -h "get_pending_votes\|Including.*vote" $LOG_DIR/node-*.log | tail -20
```

## 6. Using tmux (if nodes are running in tmux)

If nodes are running in background tmux sessions:

```bash
# List tmux sessions
tmux ls

# Attach to node 0 session
tmux attach -t node-0

# View logs from tmux
# Press Ctrl+B then [ to enter scroll mode
# Use arrow keys to scroll, q to quit
```

## 7. Check Logs After Benchmark Completes

If `log_processing: true` is set, logs are downloaded to:
```
./logs/logs-{commit}/logs-{parameters}/
```

View them:
```bash
# Find the latest log directory
ls -lt ./logs/logs-*/logs-*/ | head -1

# View a specific node log
cat ./logs/logs-*/logs-*/node-0.log | grep -E "(get_pending_votes|Including.*vote)"
```

