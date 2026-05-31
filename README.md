# lau-murmur-protocol-v2

Gossip-based information spreading with epidemic models. How rumors (and truths) spread through agent networks.

## Components

- **Murmur** — a single piece of gossip with TTL, hop count, and tags
- **MurmurType** — Fact, Rumor, Warning, Query, Instruction, Heartbeat
- **GossipAgent** — an agent that receives, forwards, and originates murmurs
- **GossipNetwork** — full network simulation with topology
- **NetworkTopology** — complete, ring, star, random (Erdős–Rényi), small-world (Watts-Strogatz)
- **EpidemicModel** — SIR model with β/γ infection/recovery rates and R₀
- **AntiEntropy** — sync protocol for eventual consistency
- **RumorTracker** — tracks truth vs rumor with corruption detection

## Usage

```rust
use lau_murmur_protocol_v2::*;

// Create a network
let topo = NetworkTopology::small_world(20, 2, 0.3);
let mut net = GossipNetwork::new(topo);

// Inject a rumor
let m = net.inject("agent-0", "the sky is falling", vec!["urgent".into()]);

// Simulate
net.run(10);
println!("Coverage: {:.1}%", net.coverage(&m.id) * 100.0);
```

## Theorems Verified

1. Complete graph: murmur reaches all agents in 1 tick
2. Ring graph: murmur takes O(n) ticks
3. Star graph: murmur reaches all in 2 ticks
4. TTL expiration stops forwarding
5. Deduplication prevents loops
6. Coverage increases monotonically
7. SIR epidemic when R₀ > 1
8. No epidemic when R₀ < 1
9. Anti-entropy full sync converges
10. Small world faster than ring
11. Random graph coverage depends on connectivity
12. More neighbors → faster coverage

## License

MIT
