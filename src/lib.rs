//! Ternary thermostat: climate control with PID, multi-zone, and scheduling.

/// Ternary thermostat state
#[derive(Clone, Debug, PartialEq)]
pub struct TernaryThermostat {
    pub target: f64,
    pub state: i8, // -1=cooling, 0=idle, +1=heating
    pub hysteresis: f64,
    pid: PidController,
}

#[derive(Clone, Debug, PartialEq)]
struct PidController {
    kp: f64, ki: f64, kd: f64,
    integral: f64,
    prev_error: f64,
    initialized: bool,
}

impl PidController {
    fn new(kp: f64, ki: f64, kd: f64) -> Self {
        Self { kp, ki, kd, integral: 0.0, prev_error: 0.0, initialized: false }
    }
    fn compute(&mut self, error: f64) -> f64 {
        self.integral += error;
        self.integral = self.integral.clamp(-100.0, 100.0);
        let deriv = if self.initialized { error - self.prev_error } else { 0.0 };
        self.prev_error = error;
        self.initialized = true;
        self.kp * error + self.ki * self.integral + self.kd * deriv
    }
}

impl TernaryThermostat {
    pub fn new(target: f64, hysteresis: f64) -> Self {
        Self { target, state: 0, hysteresis, pid: PidController::new(1.0, 0.1, 0.5) }
    }

    pub fn with_pid(mut self, kp: f64, ki: f64, kd: f64) -> Self {
        self.pid = PidController::new(kp, ki, kd);
        self
    }

    /// Sense temperature and determine desired action
    pub fn sense(&self, temp: f64, target: f64) -> i8 {
        let diff = target - temp;
        if diff > self.hysteresis { 1 }
        else if diff < -self.hysteresis { -1 }
        else { 0 }
    }

    /// Regulate with hysteresis: avoid rapid switching
    pub fn regulate(&mut self, current_state: i8, action: i8) -> i8 {
        if action == 0 { self.state = 0; return 0; }
        if current_state == action { return action; }
        // Only switch if the new action is strong enough
        self.state = action;
        action
    }

    /// PID-controlled update
    pub fn update_pid(&mut self, temp: f64) -> i8 {
        let error = self.target - temp;
        let output = self.pid.compute(error);
        let new_state = if output > self.hysteresis { 1 }
                       else if output < -self.hysteresis { -1 }
                       else { 0 };
        self.state = new_state;
        new_state
    }

    /// Cycle efficiency: fraction of time spent idle (0 state)
    pub fn cycle_efficiency(history: &[i8]) -> f64 {
        if history.is_empty() { return 0.0; }
        history.iter().filter(|&&v| v == 0).count() as f64 / history.len() as f64
    }
}

/// Multi-zone climate control
#[derive(Clone, Debug)]
pub struct Zone {
    pub id: usize,
    pub target: f64,
    pub current: f64,
    pub state: i8,
}

#[derive(Clone)]
pub struct MultiZone {
    pub zones: Vec<Zone>,
    pub coupling: f64, // heat transfer coefficient between adjacent zones
}

impl MultiZone {
    pub fn new(zones: Vec<Zone>, coupling: f64) -> Self {
        Self { zones, coupling }
    }

    /// Update all zones with heat transfer between adjacent ones
    pub fn update(&mut self, hysteresis: f64) -> Vec<i8> {
        let n = self.zones.len();
        let mut new_temps = vec![0.0f64; n];
        for i in 0..n {
            let mut heat_transfer = 0.0;
            if i > 0 { heat_transfer += self.coupling * (self.zones[i-1].current - self.zones[i].current); }
            if i < n-1 { heat_transfer += self.coupling * (self.zones[i+1].current - self.zones[i].current); }
            new_temps[i] = self.zones[i].current + heat_transfer;
        }
        let mut states = Vec::with_capacity(n);
        for i in 0..n {
            self.zones[i].current = new_temps[i];
            let diff = self.zones[i].target - self.zones[i].current;
            let state = if diff > hysteresis { 1 } else if diff < -hysteresis { -1 } else { 0 };
            self.zones[i].state = state;
            states.push(state);
        }
        states
    }
}

/// Time-based schedule for target adjustments
#[derive(Clone, Debug)]
pub struct ScheduleEntry {
    pub hour: u32,
    pub target: f64,
}

pub struct ThermostatScheduler {
    pub schedule: Vec<ScheduleEntry>,
}

impl ThermostatScheduler {
    pub fn new(schedule: Vec<ScheduleEntry>) -> Self {
        Self { schedule }
    }

    /// Get target for current hour
    pub fn target_for_hour(&self, hour: u32) -> f64 {
        let mut best = &self.schedule[0];
        for entry in &self.schedule {
            if entry.hour <= hour { best = entry; }
        }
        best.target
    }

    /// Pre-heat/cool: determine how many hours ahead to start adjusting
    pub fn pre_adjust_hours(&self, current_target: f64, next_target: f64, rate_per_hour: f64) -> f64 {
        if rate_per_hour <= 0.0 { return 0.0; }
        (next_target - current_target).abs() / rate_per_hour
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sense_heating() {
        let t = TernaryThermostat::new(22.0, 0.5);
        assert_eq!(t.sense(18.0, 22.0), 1);
    }

    #[test]
    fn test_sense_cooling() {
        let t = TernaryThermostat::new(22.0, 0.5);
        assert_eq!(t.sense(28.0, 22.0), -1);
    }

    #[test]
    fn test_sense_idle() {
        let t = TernaryThermostat::new(22.0, 0.5);
        assert_eq!(t.sense(22.1, 22.0), 0);
    }

    #[test]
    fn test_regulate() {
        let mut t = TernaryThermostat::new(22.0, 0.5);
        assert_eq!(t.regulate(0, 1), 1);
        assert_eq!(t.state, 1);
    }

    #[test]
    fn test_pid_control() {
        let mut t = TernaryThermostat::new(22.0, 0.5).with_pid(1.0, 0.1, 0.5);
        let state = t.update_pid(15.0);
        assert_eq!(state, 1); // way below target -> heat
    }

    #[test]
    fn test_cycle_efficiency() {
        let history = vec![1, 0, 0, -1, 0, 0];
        let eff = TernaryThermostat::cycle_efficiency(&history);
        assert!((eff - 4.0/6.0).abs() < 1e-10);
    }

    #[test]
    fn test_multi_zone() {
        let zones = vec![
            Zone { id: 0, target: 22.0, current: 18.0, state: 0 },
            Zone { id: 1, target: 22.0, current: 26.0, state: 0 },
        ];
        let mut mz = MultiZone::new(zones, 0.1);
        let states = mz.update(0.5);
        assert_eq!(states.len(), 2);
        assert_eq!(states[0], 1); // cold zone -> heat
        assert_eq!(states[1], -1); // hot zone -> cool
    }

    #[test]
    fn test_schedule() {
        let sched = ThermostatScheduler::new(vec![
            ScheduleEntry { hour: 0, target: 18.0 },
            ScheduleEntry { hour: 7, target: 22.0 },
            ScheduleEntry { hour: 22, target: 18.0 },
        ]);
        assert!((sched.target_for_hour(3) - 18.0).abs() < 1e-10);
        assert!((sched.target_for_hour(10) - 22.0).abs() < 1e-10);
        assert!((sched.target_for_hour(23) - 18.0).abs() < 1e-10);
    }

    #[test]
    fn test_pre_adjust() {
        let sched = ThermostatScheduler::new(vec![]);
        let hours = sched.pre_adjust_hours(18.0, 22.0, 2.0);
        assert!((hours - 2.0).abs() < 1e-10);
    }
}
