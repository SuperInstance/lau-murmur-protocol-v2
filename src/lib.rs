use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use rand::seq::SliceRandom;
use rand::Rng;

// ---------------------------------------------------------------------------
// 1. Murmur — a single piece of gossip
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Murmur {
    pub id: String,
    pub origin: String,
    pub content: String,
    pub ttl: u32,
    pub hops: u32,
    pub created_at: u64,
    pub tags: Vec<String>,
    pub murmur_type: MurmurType,
}

impl Murmur {
    pub fn is_fresh(&self) -> bool {
        self.ttl > 0
    }

    pub fn is_expired(&self) -> bool {
        self.ttl == 0
    }

    pub fn decay(&mut self) {
        if self.ttl > 0 {
            self.ttl -= 1;
        }
        self.hops += 1;
    }
}

// ---------------------------------------------------------------------------
// 2. MurmurType enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MurmurType {
    Fact,
    Rumor,
    Warning,
    Query,
    Instruction,
    Heartbeat,
}

// ---------------------------------------------------------------------------
// 5. MurmurDelivery
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MurmurDelivery {
    pub from: String,
    pub to: String,
    pub murmur: Murmur,
}

// ---------------------------------------------------------------------------
// 3. GossipAgent
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipAgent {
    pub id: String,
    pub neighbors: Vec<String>,
    pub inbox: Vec<Murmur>,
    pub outbox: Vec<(String, Murmur)>,
    pub known_murmurs: HashSet<String>,
    pub fanout: usize,
}

impl GossipAgent {
    pub fn new(id: &str, neighbors: Vec<String>) -> Self {
        Self {
            id: id.to_string(),
            neighbors,
            inbox: Vec::new(),
            outbox: Vec::new(),
            known_murmurs: HashSet::new(),
            fanout: 3,
        }
    }

    /// Receive a murmur. Returns false if duplicate or expired.
    pub fn receive(&mut self, mut murmur: Murmur) -> bool {
        if self.known_murmurs.contains(&murmur.id) {
            return false;
        }
        if murmur.is_expired() {
            return false;
        }
        self.known_murmurs.insert(murmur.id.clone());
        murmur.decay();
        self.inbox.push(murmur);
        true
    }

    /// Spread received murmurs to a random subset of neighbors.
    pub fn forward(&mut self) -> Vec<(String, Murmur)> {
        let mut deliveries = Vec::new();
        let murmurs: Vec<Murmur> = self.inbox.drain(..).collect();
        let mut rng = rand::thread_rng();

        for murmur in murmurs {
            if murmur.is_expired() {
                continue;
            }
            let n = self.fanout.min(self.neighbors.len());
            let mut targets: Vec<String> = self.neighbors.clone();
            targets.shuffle(&mut rng);
            for target in targets.iter().take(n) {
                let entry = (target.clone(), murmur.clone());
                deliveries.push(entry.clone());
                self.outbox.push(entry);
            }
        }
        deliveries
    }

    /// Originate a new murmur.
    pub fn originate(&mut self, content: &str, tags: Vec<String>) -> Murmur {
        let id = format!("{}-{}", self.id, self.known_murmurs.len());
        let murmur = Murmur {
            id: id.clone(),
            origin: self.id.clone(),
            content: content.to_string(),
            ttl: 10,
            hops: 0,
            created_at: 0,
            tags,
            murmur_type: MurmurType::Rumor,
        };
        self.known_murmurs.insert(id);
        self.inbox.push(murmur.clone());
        murmur
    }

    pub fn known_count(&self) -> usize {
        self.known_murmurs.len()
    }

    pub fn has_heard(&self, murmur_id: &str) -> bool {
        self.known_murmurs.contains(murmur_id)
    }
}

// ---------------------------------------------------------------------------
// 6. NetworkTopology
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkTopology {
    /// adjacency list: agent_id → neighbor agent_ids
    edges: HashMap<String, Vec<String>>,
}

impl NetworkTopology {
    pub fn complete(n: usize) -> Self {
        let ids: Vec<String> = (0..n).map(|i| format!("agent-{}", i)).collect();
        let mut edges = HashMap::new();
        for id in &ids {
            let neighbors: Vec<String> = ids.iter().filter(|o| *o != id).cloned().collect();
            edges.insert(id.clone(), neighbors);
        }
        Self { edges }
    }

    pub fn ring(n: usize) -> Self {
        let ids: Vec<String> = (0..n).map(|i| format!("agent-{}", i)).collect();
        let mut edges = HashMap::new();
        for (i, id) in ids.iter().enumerate() {
            let left = ids[(i + n - 1) % n].clone();
            let right = ids[(i + 1) % n].clone();
            edges.insert(id.clone(), vec![left, right]);
        }
        Self { edges }
    }

    pub fn star(n: usize) -> Self {
        let ids: Vec<String> = (0..n).map(|i| format!("agent-{}", i)).collect();
        let mut edges = HashMap::new();
        // agent-0 is the hub
        if !ids.is_empty() {
            let spokes: Vec<String> = ids.iter().skip(1).cloned().collect();
            edges.insert(ids[0].clone(), spokes);
        }
        for id in ids.iter().skip(1) {
            edges.insert(id.clone(), vec![ids[0].clone()]);
        }
        Self { edges }
    }

    pub fn random(n: usize, p: f64) -> Self {
        let ids: Vec<String> = (0..n).map(|i| format!("agent-{}", i)).collect();
        let mut edges: HashMap<String, Vec<String>> = ids.iter().map(|id| (id.clone(), Vec::new())).collect();
        let mut rng = rand::thread_rng();
        for i in 0..n {
            for j in (i + 1)..n {
                if rng.gen::<f64>() < p {
                    edges.get_mut(&ids[i]).unwrap().push(ids[j].clone());
                    edges.get_mut(&ids[j]).unwrap().push(ids[i].clone());
                }
            }
        }
        Self { edges }
    }

    pub fn small_world(n: usize, k: usize, p: f64) -> Self {
        let ids: Vec<String> = (0..n).map(|i| format!("agent-{}", i)).collect();
        let mut edges: HashMap<String, HashSet<String>> = ids.iter().map(|id| (id.clone(), HashSet::new())).collect();

        // Initial ring lattice: connect to k nearest neighbors on each side
        for i in 0..n {
            for d in 1..=k {
                let j = (i + d) % n;
                edges.get_mut(&ids[i]).unwrap().insert(ids[j].clone());
                edges.get_mut(&ids[j]).unwrap().insert(ids[i].clone());
            }
        }

        // Rewire with probability p
        let mut rng = rand::thread_rng();
        for i in 0..n {
            let neighbors: Vec<String> = edges.get(&ids[i]).unwrap().iter().cloned().collect();
            for neighbor in neighbors {
                if rng.gen::<f64>() < p {
                    // Only rewire if i < neighbor (to avoid double-rewiring)
                    let i_idx = i;
                    let n_idx = ids.iter().position(|x| x == &neighbor).unwrap();
                    if i_idx < n_idx {
                        edges.get_mut(&ids[i]).unwrap().remove(&neighbor);
                        edges.get_mut(&ids[n_idx]).unwrap().remove(&ids[i]);
                        // Pick a random target
                        let candidates: Vec<&String> = ids.iter().filter(|x| **x != ids[i] && !edges.get(&ids[i]).unwrap().contains(*x)).collect();
                        if let Some(target) = candidates.choose(&mut rng) {
                            edges.get_mut(&ids[i]).unwrap().insert((**target).clone());
                            edges.get_mut(*target).unwrap().insert(ids[i].clone());
                        } else {
                            // Re-add if no candidate
                            edges.get_mut(&ids[i]).unwrap().insert(neighbor.clone());
                            edges.get_mut(&ids[n_idx]).unwrap().insert(ids[i].clone());
                        }
                    }
                }
            }
        }

        Self {
            edges: edges.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect(),
        }
    }

    pub fn get_neighbors(&self, agent: &str) -> Vec<String> {
        self.edges.get(agent).cloned().unwrap_or_default()
    }

    pub fn agent_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.edges.keys().cloned().collect();
        ids.sort();
        ids
    }

    pub fn agent_count(&self) -> usize {
        self.edges.len()
    }

    /// Approximate diameter using BFS from all nodes.
    pub fn diameter(&self) -> usize {
        let ids: Vec<String> = self.edges.keys().cloned().collect();
        let mut max_dist = 0;
        for start in &ids {
            let mut visited: HashSet<String> = HashSet::new();
            visited.insert(start.clone());
            let mut frontier: Vec<String> = vec![start.clone()];
            let mut dist = 0;
            while !frontier.is_empty() && visited.len() < ids.len() {
                let mut next = Vec::new();
                for node in &frontier {
                    for nb in self.get_neighbors(node) {
                        if !visited.contains(&nb) {
                            visited.insert(nb.clone());
                            next.push(nb);
                        }
                    }
                }
                frontier = next;
                dist += 1;
            }
            if dist > max_dist {
                max_dist = dist;
            }
        }
        max_dist
    }
}

// ---------------------------------------------------------------------------
// 4. GossipNetwork
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipNetwork {
    pub agents: HashMap<String, GossipAgent>,
    pub topology: NetworkTopology,
    /// Track which agents know each murmur
    murmur_reach: HashMap<String, HashSet<String>>,
    /// Tick counter
    tick_count: usize,
    /// All deliveries per tick
    history: Vec<Vec<MurmurDelivery>>,
}

impl GossipNetwork {
    pub fn new(topology: NetworkTopology) -> Self {
        let mut agents = HashMap::new();
        for id in topology.agent_ids() {
            let neighbors = topology.get_neighbors(&id);
            agents.insert(id.clone(), GossipAgent::new(&id, neighbors));
        }
        Self {
            agents,
            topology,
            murmur_reach: HashMap::new(),
            tick_count: 0,
            history: Vec::new(),
        }
    }

    /// One round of gossip.
    pub fn tick(&mut self) -> Vec<MurmurDelivery> {
        let mut all_deliveries = Vec::new();

        // Collect all forwards from all agents
        let mut pending: Vec<MurmurDelivery> = Vec::new();
        for agent in self.agents.values_mut() {
            let forwards = agent.forward();
            for (target, murmur) in forwards {
                pending.push(MurmurDelivery {
                    from: agent.id.clone(),
                    to: target,
                    murmur,
                });
            }
        }

        // Deliver
        for delivery in pending {
            if let Some(target_agent) = self.agents.get_mut(&delivery.to) {
                let murmur_id = delivery.murmur.id.clone();
                let accepted = target_agent.receive(delivery.murmur.clone());
                if accepted {
                    self.murmur_reach.entry(murmur_id.clone()).or_default().insert(delivery.to.clone());
                }
                all_deliveries.push(MurmurDelivery {
                    from: delivery.from,
                    to: delivery.to,
                    murmur: delivery.murmur,
                });
            }
        }

        self.tick_count += 1;
        self.history.push(all_deliveries.clone());
        all_deliveries
    }

    /// Inject a new murmur from a specific agent.
    pub fn inject(&mut self, origin: &str, content: &str, tags: Vec<String>) -> Murmur {
        let murmur = if let Some(agent) = self.agents.get_mut(origin) {
            let m = agent.originate(content, tags);
            self.murmur_reach.entry(m.id.clone()).or_default().insert(origin.to_string());
            m
        } else {
            panic!("Agent {} not found", origin);
        };
        murmur
    }

    /// Run simulation for given steps.
    pub fn run(&mut self, steps: usize) -> Vec<Vec<MurmurDelivery>> {
        let mut results = Vec::new();
        for _ in 0..steps {
            results.push(self.tick());
        }
        results
    }

    /// Fraction of agents that heard a specific murmur.
    pub fn coverage(&self, murmur_id: &str) -> f64 {
        let total = self.agents.len() as f64;
        if total == 0.0 {
            return 0.0;
        }
        let reached = self.murmur_reach.get(murmur_id).map(|s| s.len()).unwrap_or(0);
        reached as f64 / total
    }

    /// Average coverage across all murmurs.
    pub fn total_coverage(&self) -> f64 {
        if self.murmur_reach.is_empty() {
            return 0.0;
        }
        let total = self.agents.len() as f64;
        let sum: f64 = self.murmur_reach.values().map(|s| s.len() as f64 / total).sum();
        sum / self.murmur_reach.len() as f64
    }

    /// Steps until 95% coverage for any murmur (earliest such tick).
    pub fn spread_speed(&self) -> usize {
        let total = self.agents.len() as f64;
        let threshold = 0.95 * total;
        for (tick, deliveries) in self.history.iter().enumerate() {
            // Check coverage after this tick
            let mut reached: HashSet<String> = HashSet::new();
            for d in deliveries {
                reached.insert(d.to.clone());
            }
            // Actually we need cumulative reach per murmur
            // Use murmur_reach which is always up to date
            for agents in self.murmur_reach.values() {
                if agents.len() as f64 >= threshold {
                    return tick + 1;
                }
            }
        }
        // If we're here, check if any murmur hit 95% at current state
        for agents in self.murmur_reach.values() {
            if agents.len() as f64 >= threshold {
                return self.history.len();
            }
        }
        usize::MAX
    }

    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }
}

// ---------------------------------------------------------------------------
// 7. EpidemicModel — SIR model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpidemicModel {
    pub susceptible: usize,
    pub infected: usize,
    pub recovered: usize,
    pub beta: f64,   // infection rate
    pub gamma: f64,  // recovery rate
    pub population: usize,
    pub history: Vec<(usize, usize, usize)>,
}

impl EpidemicModel {
    pub fn new(population: usize, initial_infected: usize, beta: f64, gamma: f64) -> Self {
        Self {
            susceptible: population - initial_infected,
            infected: initial_infected,
            recovered: 0,
            beta,
            gamma,
            population,
            history: vec![(population - initial_infected, initial_infected, 0)],
        }
    }

    pub fn step(&mut self) -> (usize, usize, usize) {
        let s = self.susceptible as f64;
        let i = self.infected as f64;
        let n = self.population as f64;

        let new_infected = (self.beta * s * i / n).round() as usize;
        let new_recovered = (self.gamma * i).round() as usize;

        self.susceptible = self.susceptible.saturating_sub(new_infected);
        self.infected += new_infected;
        self.infected -= new_recovered;
        self.recovered += new_recovered;

        // Clamp
        self.infected = self.infected.min(self.population);
        self.recovered = self.recovered.min(self.population);
        if self.susceptible + self.infected + self.recovered > self.population {
            self.susceptible = self.population - self.infected - self.recovered;
        }

        let state = (self.susceptible, self.infected, self.recovered);
        self.history.push(state);
        state
    }

    pub fn run(&mut self, steps: usize) -> Vec<(usize, usize, usize)> {
        for _ in 0..steps {
            self.step();
        }
        self.history.clone()
    }

    /// Basic reproduction number.
    pub fn r0(&self) -> f64 {
        if self.gamma == 0.0 {
            return f64::INFINITY;
        }
        self.beta / self.gamma
    }

    /// Peak infection count.
    pub fn peak_infection(&self) -> usize {
        self.history.iter().map(|(_, i, _)| *i).max().unwrap_or(0)
    }

    /// True when r0 > 1.
    pub fn is_epidemic(&self) -> bool {
        self.r0() > 1.0
    }
}

// ---------------------------------------------------------------------------
// 8. AntiEntropy — sync protocol for eventual consistency
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiEntropy {
    pub agent_states: HashMap<String, Vec<String>>,
}

impl AntiEntropy {
    pub fn new(agent_ids: &[String]) -> Self {
        Self {
            agent_states: agent_ids.iter().map(|id| (id.clone(), Vec::new())).collect(),
        }
    }

    /// Add a murmur to an agent's known set.
    pub fn add_murmur(&mut self, agent: &str, murmur_id: &str) {
        if let Some(state) = self.agent_states.get_mut(agent) {
            if !state.contains(&murmur_id.to_string()) {
                state.push(murmur_id.to_string());
            }
        }
    }

    /// Sync two agents: returns what a gives to b and vice versa.
    pub fn sync_pair(&mut self, a: &str, b: &str) -> (Vec<String>, Vec<String>) {
        let a_set: HashSet<String> = self.agent_states.get(a).unwrap().iter().cloned().collect();
        let b_set: HashSet<String> = self.agent_states.get(b).unwrap().iter().cloned().collect();

        let a_to_b: Vec<String> = a_set.difference(&b_set).cloned().collect();
        let b_to_a: Vec<String> = b_set.difference(&a_set).cloned().collect();

        // Apply
        if let Some(state) = self.agent_states.get_mut(b) {
            for id in &a_to_b {
                if !state.contains(id) {
                    state.push(id.clone());
                }
            }
        }
        if let Some(state) = self.agent_states.get_mut(a) {
            for id in &b_to_a {
                if !state.contains(id) {
                    state.push(id.clone());
                }
            }
        }

        (a_to_b, b_to_a)
    }

    /// Full sync all pairs. Returns total number of exchanges.
    pub fn full_sync(&mut self) -> usize {
        let agents: Vec<String> = self.agent_states.keys().cloned().collect();
        let mut total = 0;
        for i in 0..agents.len() {
            for j in (i + 1)..agents.len() {
                let (a, b) = self.sync_pair(&agents[i], &agents[j]);
                total += a.len() + b.len();
            }
        }
        total
    }

    /// True when all agents have the same state.
    pub fn is_converged(&self) -> bool {
        let states: Vec<&Vec<String>> = self.agent_states.values().collect();
        if states.len() <= 1 {
            return true;
        }
        let first: HashSet<&String> = states[0].iter().collect();
        states.iter().all(|s| {
            let set: HashSet<&String> = s.iter().collect();
            set == first
        })
    }
}

// ---------------------------------------------------------------------------
// 9 & 10. RumorTracker
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MurmurRecord {
    pub origin: String,
    pub content: String,
    pub is_true: bool,
    pub mutations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RumorTracker {
    pub murmurs: HashMap<String, MurmurRecord>,
    pub original_content: HashMap<String, String>,
}

impl RumorTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, murmur: &Murmur, is_true: bool) {
        let is_new = !self.murmurs.contains_key(&murmur.id);
        let mutations = if is_new { 0 } else { self.murmurs[&murmur.id].mutations };

        if is_new {
            self.original_content.insert(murmur.id.clone(), murmur.content.clone());
        }

        // Check if content changed from original
        let orig = self.original_content.get(&murmur.id).unwrap();
        let mutations = if &murmur.content != orig { mutations + 1 } else { mutations };

        self.murmurs.insert(
            murmur.id.clone(),
            MurmurRecord {
                origin: murmur.origin.clone(),
                content: murmur.content.clone(),
                is_true,
                mutations,
            },
        );
    }

    /// How much content changed (fraction of murmurs with mutations).
    pub fn corrupted(&self, murmur_id: &str) -> f64 {
        match self.murmurs.get(murmur_id) {
            Some(r) => {
                let orig = self.original_content.get(murmur_id).unwrap();
                if r.content == *orig { 0.0 } else { 1.0 }
            }
            None => 0.0,
        }
    }

    /// Fraction of recorded murmurs that are true.
    pub fn truth_ratio(&self) -> f64 {
        if self.murmurs.is_empty() {
            return 1.0;
        }
        let true_count = self.murmurs.values().filter(|r| r.is_true).count();
        true_count as f64 / self.murmurs.len() as f64
    }
}

// ---------------------------------------------------------------------------
// lib / main
// ---------------------------------------------------------------------------

pub fn greet() -> String {
    "murmur protocol v2 — gossip-based information spreading".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Murmur tests ----

    #[test]
    fn test_murmur_fresh() {
        let m = Murmur {
            id: "m1".into(), origin: "a".into(), content: "hello".into(),
            ttl: 3, hops: 0, created_at: 0, tags: vec![], murmur_type: MurmurType::Rumor,
        };
        assert!(m.is_fresh());
        assert!(!m.is_expired());
    }

    #[test]
    fn test_murmur_expired() {
        let m = Murmur {
            id: "m1".into(), origin: "a".into(), content: "hello".into(),
            ttl: 0, hops: 5, created_at: 0, tags: vec![], murmur_type: MurmurType::Fact,
        };
        assert!(!m.is_fresh());
        assert!(m.is_expired());
    }

    #[test]
    fn test_murmur_decay() {
        let mut m = Murmur {
            id: "m1".into(), origin: "a".into(), content: "hello".into(),
            ttl: 2, hops: 0, created_at: 0, tags: vec![], murmur_type: MurmurType::Rumor,
        };
        m.decay();
        assert_eq!(m.ttl, 1);
        assert_eq!(m.hops, 1);
        m.decay();
        assert_eq!(m.ttl, 0);
        assert_eq!(m.hops, 2);
        assert!(m.is_expired());
    }

    #[test]
    fn test_murmur_decay_does_not_underflow() {
        let mut m = Murmur {
            id: "m1".into(), origin: "a".into(), content: "hello".into(),
            ttl: 0, hops: 5, created_at: 0, tags: vec![], murmur_type: MurmurType::Rumor,
        };
        m.decay();
        assert_eq!(m.ttl, 0);
    }

    #[test]
    fn test_murmur_serde_roundtrip() {
        let m = Murmur {
            id: "m1".into(), origin: "a".into(), content: "hello".into(),
            ttl: 3, hops: 0, created_at: 123, tags: vec!["tag1".into()],
            murmur_type: MurmurType::Warning,
        };
        let json = serde_json::to_string(&m).unwrap();
        let m2: Murmur = serde_json::from_str(&json).unwrap();
        assert_eq!(m, m2);
    }

    #[test]
    fn test_murmur_type_serde() {
        for mt in [MurmurType::Fact, MurmurType::Rumor, MurmurType::Warning,
                    MurmurType::Query, MurmurType::Instruction, MurmurType::Heartbeat] {
            let json = serde_json::to_string(&mt).unwrap();
            let mt2: MurmurType = serde_json::from_str(&json).unwrap();
            assert_eq!(mt, mt2);
        }
    }

    // ---- GossipAgent tests ----

    #[test]
    fn test_agent_receive() {
        let mut agent = GossipAgent::new("a", vec!["b".into(), "c".into()]);
        let m = Murmur {
            id: "m1".into(), origin: "b".into(), content: "hi".into(),
            ttl: 5, hops: 0, created_at: 0, tags: vec![], murmur_type: MurmurType::Rumor,
        };
        assert!(agent.receive(m));
        assert!(agent.has_heard("m1"));
        assert_eq!(agent.known_count(), 1);
    }

    #[test]
    fn test_agent_reject_duplicate() {
        let mut agent = GossipAgent::new("a", vec!["b".into()]);
        let m = Murmur {
            id: "m1".into(), origin: "b".into(), content: "hi".into(),
            ttl: 5, hops: 0, created_at: 0, tags: vec![], murmur_type: MurmurType::Rumor,
        };
        assert!(agent.receive(m.clone()));
        assert!(!agent.receive(m));
    }

    #[test]
    fn test_agent_reject_expired() {
        let mut agent = GossipAgent::new("a", vec![]);
        let m = Murmur {
            id: "m1".into(), origin: "b".into(), content: "hi".into(),
            ttl: 0, hops: 5, created_at: 0, tags: vec![], murmur_type: MurmurType::Rumor,
        };
        assert!(!agent.receive(m));
    }

    #[test]
    fn test_agent_forward() {
        let mut agent = GossipAgent::new("a", vec!["b".into(), "c".into(), "d".into()]);
        agent.fanout = 2;
        let m = Murmur {
            id: "m1".into(), origin: "b".into(), content: "hi".into(),
            ttl: 5, hops: 0, created_at: 0, tags: vec![], murmur_type: MurmurType::Rumor,
        };
        agent.receive(m);
        let deliveries = agent.forward();
        assert_eq!(deliveries.len(), 2);
    }

    #[test]
    fn test_agent_originate() {
        let mut agent = GossipAgent::new("a", vec![]);
        let m = agent.originate("breaking news", vec!["urgent".into()]);
        assert_eq!(m.origin, "a");
        assert_eq!(m.content, "breaking news");
        assert!(agent.has_heard(&m.id));
    }

    #[test]
    fn test_agent_forward_no_expired() {
        let mut agent = GossipAgent::new("a", vec!["b".into()]);
        // Receive a murmur with ttl=1, it will decay to 0
        let m = Murmur {
            id: "m1".into(), origin: "b".into(), content: "hi".into(),
            ttl: 1, hops: 0, created_at: 0, tags: vec![], murmur_type: MurmurType::Rumor,
        };
        agent.receive(m);
        // After receive, ttl is now 0 (decayed from 1 to 0)
        let deliveries = agent.forward();
        assert!(deliveries.is_empty()); // expired, won't forward
    }

    // ---- NetworkTopology tests ----

    #[test]
    fn test_complete_graph() {
        let topo = NetworkTopology::complete(4);
        assert_eq!(topo.agent_count(), 4);
        for i in 0..4 {
            let id = format!("agent-{}", i);
            assert_eq!(topo.get_neighbors(&id).len(), 3);
        }
    }

    #[test]
    fn test_ring_graph() {
        let topo = NetworkTopology::ring(5);
        assert_eq!(topo.agent_count(), 5);
        for i in 0..5 {
            let id = format!("agent-{}", i);
            assert_eq!(topo.get_neighbors(&id).len(), 2);
        }
    }

    #[test]
    fn test_star_graph() {
        let topo = NetworkTopology::star(5);
        assert_eq!(topo.agent_count(), 5);
        assert_eq!(topo.get_neighbors("agent-0").len(), 4);
        assert_eq!(topo.get_neighbors("agent-1").len(), 1);
    }

    #[test]
    fn test_random_graph() {
        let topo = NetworkTopology::random(10, 0.5);
        assert_eq!(topo.agent_count(), 10);
    }

    #[test]
    fn test_small_world_graph() {
        let topo = NetworkTopology::small_world(10, 2, 0.3);
        assert_eq!(topo.agent_count(), 10);
    }

    #[test]
    fn test_diameter_complete() {
        let topo = NetworkTopology::complete(5);
        assert_eq!(topo.diameter(), 1);
    }

    #[test]
    fn test_diameter_ring() {
        let topo = NetworkTopology::ring(6);
        assert_eq!(topo.diameter(), 3);
    }

    #[test]
    fn test_diameter_star() {
        let topo = NetworkTopology::star(5);
        assert_eq!(topo.diameter(), 2);
    }

    // ---- GossipNetwork tests ----

    #[test]
    fn test_theorem_1_complete_graph_one_tick() {
        // Complete graph: murmur reaches all agents in 1 tick
        let topo = NetworkTopology::complete(5);
        let mut net = GossipNetwork::new(topo);
        let m = net.inject("agent-0", "hello", vec![]);
        // Set high fanout for all agents
        for agent in net.agents.values_mut() {
            agent.fanout = 10;
        }
        net.tick();
        // All agents should know the murmur
        for (id, agent) in &net.agents {
            if id != "agent-0" {
                assert!(agent.has_heard(&m.id), "agent {} should have heard {}", id, m.id);
            }
        }
        assert!((net.coverage(&m.id) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_theorem_3_star_two_ticks() {
        // Star: murmur from spoke → hub → all spokes
        let topo = NetworkTopology::star(5);
        let mut net = GossipNetwork::new(topo);
        for agent in net.agents.values_mut() {
            agent.fanout = 10;
        }
        let m = net.inject("agent-1", "news", vec![]);
        net.tick(); // agent-1 forwards to hub (agent-0)
        net.tick(); // hub forwards to all spokes
        // All should know
        for agent in net.agents.values() {
            assert!(agent.has_heard(&m.id));
        }
    }

    #[test]
    fn test_theorem_4_ttl_expiration() {
        // TTL expiration stops forwarding
        let topo = NetworkTopology::ring(10);
        let mut net = GossipNetwork::new(topo);
        // Inject with TTL=1
        let agent = net.agents.get_mut("agent-0").unwrap();
        let murmur = Murmur {
            id: "ttl-test".into(), origin: "agent-0".into(), content: "short-lived".into(),
            ttl: 1, hops: 0, created_at: 0, tags: vec![], murmur_type: MurmurType::Rumor,
        };
        agent.known_murmurs.insert("ttl-test".into());
        agent.inbox.push(murmur);
        net.murmur_reach.insert("ttl-test".into(), {
            let mut s = HashSet::new();
            s.insert("agent-0".into());
            s
        });

        // After 1 tick, the murmur decays to ttl=0 in the receiver, so it won't spread further
        net.tick();
        let coverage_after_1 = net.coverage("ttl-test");
        assert!(coverage_after_1 < 1.0, "TTL=1 should not reach everyone in a ring of 10");
    }

    #[test]
    fn test_theorem_5_deduplication() {
        // Deduplication prevents loops
        let topo = NetworkTopology::ring(3);
        let mut net = GossipNetwork::new(topo);
        for agent in net.agents.values_mut() {
            agent.fanout = 10;
        }
        let _m = net.inject("agent-0", "no loops", vec![]);
        // Run many ticks
        for _ in 0..20 {
            net.tick();
        }
        // Verify no agent received duplicate (known_count should be limited)
        for agent in net.agents.values() {
            assert!(agent.known_count() <= 1); // only one unique murmur
        }
    }

    #[test]
    fn test_theorem_6_coverage_monotonic() {
        // Coverage increases monotonically
        let topo = NetworkTopology::complete(5);
        let mut net = GossipNetwork::new(topo);
        for agent in net.agents.values_mut() {
            agent.fanout = 10;
        }
        let m = net.inject("agent-0", "monotonic", vec![]);
        let mut prev_coverage = 0.0;
        for _ in 0..10 {
            net.tick();
            let cov = net.coverage(&m.id);
            assert!(cov >= prev_coverage, "coverage should not decrease");
            prev_coverage = cov;
        }
    }

    #[test]
    fn test_inject_and_coverage() {
        let topo = NetworkTopology::complete(3);
        let mut net = GossipNetwork::new(topo);
        for agent in net.agents.values_mut() {
            agent.fanout = 10;
        }
        let m = net.inject("agent-0", "test", vec![]);
        assert!((net.coverage(&m.id) - (1.0 / 3.0)).abs() < 0.01);
        net.tick();
        assert!((net.coverage(&m.id) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_run_simulation() {
        let topo = NetworkTopology::ring(6);
        let mut net = GossipNetwork::new(topo);
        for agent in net.agents.values_mut() {
            agent.fanout = 10;
        }
        net.inject("agent-0", "ring gossip", vec![]);
        let results = net.run(20);
        assert_eq!(results.len(), 20);
    }

    #[test]
    fn test_total_coverage() {
        let topo = NetworkTopology::complete(3);
        let mut net = GossipNetwork::new(topo);
        for agent in net.agents.values_mut() {
            agent.fanout = 10;
        }
        net.inject("agent-0", "msg1", vec![]);
        net.tick();
        assert!((net.total_coverage() - 1.0).abs() < 0.01);
    }

    // ---- EpidemicModel tests ----

    #[test]
    fn test_theorem_7_epidemic_r0_gt_1() {
        let model = EpidemicModel::new(1000, 10, 0.3, 0.1);
        assert!(model.is_epidemic());
        assert!(model.r0() > 1.0);
    }

    #[test]
    fn test_theorem_8_no_epidemic_r0_lt_1() {
        let model = EpidemicModel::new(1000, 10, 0.1, 0.3);
        assert!(!model.is_epidemic());
        assert!(model.r0() < 1.0);
    }

    #[test]
    fn test_sir_population_conservation() {
        let mut model = EpidemicModel::new(1000, 10, 0.3, 0.1);
        for _ in 0..100 {
            model.step();
        }
        assert_eq!(model.susceptible + model.infected + model.recovered, 1000);
    }

    #[test]
    fn test_sir_run() {
        let mut model = EpidemicModel::new(1000, 10, 0.3, 0.1);
        let history = model.run(50);
        assert_eq!(history.len(), 51); // initial + 50 steps
    }

    #[test]
    fn test_sir_peak_infection() {
        let mut model = EpidemicModel::new(1000, 10, 0.3, 0.1);
        model.run(100);
        let peak = model.peak_infection();
        assert!(peak >= 10);
    }

    #[test]
    fn test_sir_r0() {
        let model = EpidemicModel::new(100, 1, 2.0, 1.0);
        assert!((model.r0() - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_epidemic_model_serde() {
        let model = EpidemicModel::new(100, 5, 0.3, 0.1);
        let json = serde_json::to_string(&model).unwrap();
        let m2: EpidemicModel = serde_json::from_str(&json).unwrap();
        assert_eq!(model.population, m2.population);
    }

    // ---- AntiEntropy tests ----

    #[test]
    fn test_theorem_9_anti_entropy_convergence() {
        let ids: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let mut ae = AntiEntropy::new(&ids);
        ae.add_murmur("a", "m1");
        ae.add_murmur("b", "m2");
        ae.add_murmur("c", "m3");
        assert!(!ae.is_converged());
        ae.full_sync();
        assert!(ae.is_converged());
        // All should have all 3
        for state in ae.agent_states.values() {
            assert_eq!(state.len(), 3);
        }
    }

    #[test]
    fn test_sync_pair() {
        let ids: Vec<String> = vec!["a".into(), "b".into()];
        let mut ae = AntiEntropy::new(&ids);
        ae.add_murmur("a", "m1");
        ae.add_murmur("a", "m2");
        ae.add_murmur("b", "m3");
        let (a_to_b, b_to_a) = ae.sync_pair("a", "b");
        assert_eq!(a_to_b.len(), 2); // m1, m2
        assert_eq!(b_to_a.len(), 1); // m3
        assert!(ae.is_converged());
    }

    #[test]
    fn test_anti_entropy_already_converged() {
        let ids: Vec<String> = vec!["a".into(), "b".into()];
        let mut ae = AntiEntropy::new(&ids);
        ae.add_murmur("a", "m1");
        ae.add_murmur("b", "m1");
        assert!(ae.is_converged());
    }

    #[test]
    fn test_anti_entropy_empty() {
        let ids: Vec<String> = vec!["a".into(), "b".into()];
        let ae = AntiEntropy::new(&ids);
        assert!(ae.is_converged());
    }

    #[test]
    fn test_full_sync_returns_exchanges() {
        let ids: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let mut ae = AntiEntropy::new(&ids);
        ae.add_murmur("a", "m1");
        let total = ae.full_sync();
        assert!(total > 0);
        assert!(ae.is_converged());
    }

    // ---- RumorTracker tests ----

    #[test]
    fn test_rumor_tracker_record() {
        let mut tracker = RumorTracker::new();
        let m = Murmur {
            id: "m1".into(), origin: "a".into(), content: "hello".into(),
            ttl: 5, hops: 0, created_at: 0, tags: vec![], murmur_type: MurmurType::Rumor,
        };
        tracker.record(&m, true);
        assert!((tracker.truth_ratio() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_rumor_tracker_truth_ratio() {
        let mut tracker = RumorTracker::new();
        for i in 0..4 {
            let m = Murmur {
                id: format!("m{}", i), origin: "a".into(), content: format!("msg{}", i),
                ttl: 5, hops: 0, created_at: 0, tags: vec![], murmur_type: MurmurType::Rumor,
            };
            tracker.record(&m, i % 2 == 0);
        }
        assert!((tracker.truth_ratio() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_rumor_tracker_corrupted() {
        let mut tracker = RumorTracker::new();
        let m = Murmur {
            id: "m1".into(), origin: "a".into(), content: "original".into(),
            ttl: 5, hops: 0, created_at: 0, tags: vec![], murmur_type: MurmurType::Rumor,
        };
        tracker.record(&m, true);
        assert!((tracker.corrupted("m1") - 0.0).abs() < 0.01);

        // Record mutated version
        let m2 = Murmur {
            id: "m1".into(), origin: "b".into(), content: "mutated".into(),
            ttl: 5, hops: 2, created_at: 0, tags: vec![], murmur_type: MurmurType::Rumor,
        };
        tracker.record(&m2, false);
        assert!((tracker.corrupted("m1") - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_rumor_tracker_unknown_murmur() {
        let tracker = RumorTracker::new();
        assert!((tracker.corrupted("nonexistent") - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_rumor_tracker_empty_ratio() {
        let tracker = RumorTracker::new();
        assert!((tracker.truth_ratio() - 1.0).abs() < 0.01);
    }

    // ---- Theorem 10: Small world faster than ring ----

    #[test]
    fn test_theorem_10_small_world_faster_than_ring() {
        let n = 20;
        let ring = NetworkTopology::ring(n);
        let sw = NetworkTopology::small_world(n, 2, 0.3);

        let mut ring_net = GossipNetwork::new(ring);
        let mut sw_net = GossipNetwork::new(sw);
        for agent in ring_net.agents.values_mut() { agent.fanout = 10; }
        for agent in sw_net.agents.values_mut() { agent.fanout = 10; }

        ring_net.inject("agent-0", "ring-msg", vec![]);
        sw_net.inject("agent-0", "sw-msg", vec![]);

        let mut ring_steps = 0;
        let mut sw_steps = 0;

        for _ in 0..100 {
            ring_net.tick();
            ring_steps += 1;
            if ring_net.total_coverage() >= 0.95 { break; }
        }
        for _ in 0..100 {
            sw_net.tick();
            sw_steps += 1;
            if sw_net.total_coverage() >= 0.95 { break; }
        }

        assert!(sw_steps <= ring_steps, "small world should spread at least as fast as ring");
    }

    // ---- Theorem 11: Random graph coverage depends on connectivity ----

    #[test]
    fn test_theorem_11_random_graph_connectivity() {
        // High connectivity → high coverage
        let topo = NetworkTopology::random(10, 0.8);
        let mut net = GossipNetwork::new(topo);
        for agent in net.agents.values_mut() { agent.fanout = 10; }
        net.inject("agent-0", "well-connected", vec![]);
        net.run(20);
        assert!(net.total_coverage() > 0.5);
    }

    // ---- Theorem 12: More neighbors → faster coverage ----

    #[test]
    fn test_theorem_12_more_neighbors_faster() {
        let complete = NetworkTopology::complete(10);
        let ring = NetworkTopology::ring(10);

        let mut c_net = GossipNetwork::new(complete);
        let mut r_net = GossipNetwork::new(ring);
        for agent in c_net.agents.values_mut() { agent.fanout = 10; }
        for agent in r_net.agents.values_mut() { agent.fanout = 10; }

        c_net.inject("agent-0", "c-msg", vec![]);
        r_net.inject("agent-0", "r-msg", vec![]);

        c_net.tick();
        r_net.tick();

        assert!(c_net.total_coverage() >= r_net.total_coverage(),
            "complete graph should have higher coverage after 1 tick than ring");
    }

    // ---- Additional tests for coverage ----

    #[test]
    fn test_gossip_network_agent_count() {
        let topo = NetworkTopology::complete(7);
        let net = GossipNetwork::new(topo);
        assert_eq!(net.agent_count(), 7);
    }

    #[test]
    fn test_network_topology_serde() {
        let topo = NetworkTopology::complete(3);
        let json = serde_json::to_string(&topo).unwrap();
        let t2: NetworkTopology = serde_json::from_str(&json).unwrap();
        assert_eq!(topo.agent_count(), t2.agent_count());
    }

    #[test]
    fn test_gossip_agent_serde() {
        let agent = GossipAgent::new("test", vec!["a".into()]);
        let json = serde_json::to_string(&agent).unwrap();
        let a2: GossipAgent = serde_json::from_str(&json).unwrap();
        assert_eq!(agent.id, a2.id);
    }

    #[test]
    fn test_murmur_delivery_serde() {
        let d = MurmurDelivery {
            from: "a".into(), to: "b".into(),
            murmur: Murmur {
                id: "m1".into(), origin: "a".into(), content: "hi".into(),
                ttl: 5, hops: 0, created_at: 0, tags: vec![], murmur_type: MurmurType::Fact,
            },
        };
        let json = serde_json::to_string(&d).unwrap();
        let d2: MurmurDelivery = serde_json::from_str(&json).unwrap();
        assert_eq!(d, d2);
    }

    #[test]
    fn test_anti_entropy_serde() {
        let ids: Vec<String> = vec!["a".into(), "b".into()];
        let ae = AntiEntropy::new(&ids);
        let json = serde_json::to_string(&ae).unwrap();
        let ae2: AntiEntropy = serde_json::from_str(&json).unwrap();
        assert_eq!(ae.agent_states.len(), ae2.agent_states.len());
    }

    #[test]
    fn test_rumor_tracker_serde() {
        let tracker = RumorTracker::new();
        let json = serde_json::to_string(&tracker).unwrap();
        let t2: RumorTracker = serde_json::from_str(&json).unwrap();
        assert_eq!(tracker.murmurs.len(), t2.murmurs.len());
    }

    #[test]
    fn test_ring_spreads_eventually() {
        let topo = NetworkTopology::ring(6);
        let mut net = GossipNetwork::new(topo);
        for agent in net.agents.values_mut() { agent.fanout = 10; }
        net.inject("agent-0", "eventual", vec![]);
        net.run(20);
        // Should reach most agents eventually
        assert!(net.total_coverage() > 0.5);
    }

    #[test]
    fn test_epidemic_model_no_infection_when_subcritical() {
        let mut model = EpidemicModel::new(1000, 5, 0.05, 0.5);
        model.run(50);
        // With r0 = 0.1, infection should die out quickly
        assert!(model.peak_infection() < 50);
    }

    #[test]
    fn test_complete_network_spread_speed() {
        let topo = NetworkTopology::complete(5);
        let mut net = GossipNetwork::new(topo);
        for agent in net.agents.values_mut() { agent.fanout = 10; }
        net.inject("agent-0", "fast", vec![]);
        net.run(5);
        // Should reach 95%+ quickly
        assert!(net.total_coverage() >= 0.95);
    }

    #[test]
    fn test_star_spoke_to_spoke() {
        let topo = NetworkTopology::star(4);
        let mut net = GossipNetwork::new(topo);
        for agent in net.agents.values_mut() { agent.fanout = 10; }
        let m = net.inject("agent-1", "spoke gossip", vec![]);
        net.tick(); // spoke → hub
        net.tick(); // hub → all spokes
        assert!(net.agents.get("agent-2").unwrap().has_heard(&m.id));
        assert!(net.agents.get("agent-3").unwrap().has_heard(&m.id));
    }

    #[test]
    fn test_gossip_network_empty_coverage() {
        let topo = NetworkTopology::complete(3);
        let net = GossipNetwork::new(topo);
        assert!((net.total_coverage() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_network_spread_speed_no_murmurs() {
        let topo = NetworkTopology::complete(3);
        let net = GossipNetwork::new(topo);
        assert_eq!(net.spread_speed(), usize::MAX);
    }

    #[test]
    fn test_multiple_murmurs() {
        let topo = NetworkTopology::complete(4);
        let mut net = GossipNetwork::new(topo);
        for agent in net.agents.values_mut() { agent.fanout = 10; }
        net.inject("agent-0", "msg1", vec![]);
        net.inject("agent-1", "msg2", vec![]);
        net.tick();
        for agent in net.agents.values() {
            assert_eq!(agent.known_count(), 2);
        }
    }

    #[test]
    fn test_greet() {
        assert!(greet().contains("murmur"));
    }
}
