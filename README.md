# ternary-thermostat

Climate control where the system is always in one of three states: cooling, idle, or heating.

## Why This Exists

A thermostat doesn't need to know *how much* to heat or cool. It needs to know *which direction* and *whether to bother*. Traditional PID-controlled HVAC systems compute a continuous output, then quantize it into on/off relay states anyway. The ternary thermostat short-circuits that dance: the control variable is `{-1, 0, +1}` from the start. Cooling, idle, heating. That's the hardware reality — so make it the software reality too.

The non-obvious win is **hysteresis**. Without it, a thermostat sitting near the setpoint will rapidly cycle between heating and cooling, destroying compressor life. With ternary + hysteresis, there's a dead zone where the system coasts. The PID controller respects this dead zone while still tracking the setpoint over time.

Multi-zone support adds another dimension: adjacent zones exchange heat through walls. Ignoring coupling means fighting your own building. The `MultiZone` struct models heat transfer between neighbors, so zone 1 heating doesn't accidentally undo zone 2's cooling.

## Architecture

```
            ┌──────────────┐
Temperature ─►│ Sense(temp)  │──► i8 {-1, 0, +1}
            │  (hysteresis) │
            └──────────────┘
                    │
            ┌───────▼───────┐
            │  PID Controller│──► continuous output → ternary quantize
            │  (kp, ki, kd) │
            └───────┬───────┘
                    │
    ┌───────────────┼───────────────┐
    │           MultiZone           │
    │  Zone 0 ←coupling→ Zone 1    │
    │  (heat transfer between       │
    │   adjacent zones)             │
    └───────────────────────────────┘
                    │
        ThermostatScheduler
        (time-of-day target adjustments)
```

**Key types:**

- **`TernaryThermostat`** — core state machine. Holds target temp, current state `{-1, 0, +1}`, hysteresis band, and internal PID controller. `sense()` reads temperature and returns raw action; `regulate()` applies hysteresis; `update_pid()` runs full PID control loop.
- **`PidController`** — internal PID with integral clamping. Anti-windup prevents the integral term from accumulating when the system is saturated.
- **`Zone`** — a single climate zone with target, current temperature, and state.
- **`MultiZone`** — collection of zones with a coupling coefficient. Each `update()` step simulates heat transfer between adjacent zones before deciding actions.
- **`ThermostatScheduler`** — time-based target adjustments (e.g., 18°C night, 22°C day). Computes pre-heating/pre-cooling lead time based on temperature rate-of-change.

## Usage

```rust
use ternary_thermostat::{TernaryThermostat, Zone, MultiZone, ThermostatScheduler, ScheduleEntry};

// Basic thermostat with hysteresis
let mut t = TernaryThermostat::new(22.0, 0.5);
assert_eq!(t.sense(18.0, 22.0), 1);   // heat
assert_eq!(t.sense(28.0, 22.0), -1);  // cool
assert_eq!(t.sense(22.1, 22.0), 0);   // idle (within hysteresis)

// PID-controlled updates
let mut t = TernaryThermostat::new(22.0, 0.5).with_pid(1.0, 0.1, 0.5);
let state = t.update_pid(15.0); // way below target → heating (+1)
assert_eq!(state, 1);

// Regulate with hysteresis to prevent rapid switching
let action = t.regulate(1, -1); // was heating, wants to cool → switch

// Measure cycle efficiency (fraction of time idle)
let history = vec![1, 0, 0, -1, 0, 0];
let efficiency = TernaryThermostat::cycle_efficiency(&history); // 0.667

// Multi-zone climate control
let zones = vec![
    Zone { id: 0, target: 22.0, current: 18.0, state: 0 },
    Zone { id: 1, target: 22.0, current: 26.0, state: 0 },
];
let mut mz = MultiZone::new(zones, 0.1); // 0.1 coupling coefficient
let states = mz.update(0.5); // [1, -1] — heat zone 0, cool zone 1

// Scheduled target adjustments
let sched = ThermostatScheduler::new(vec![
    ScheduleEntry { hour: 0,  target: 18.0 },  // night
    ScheduleEntry { hour: 7,  target: 22.0 },  // morning
    ScheduleEntry { hour: 22, target: 18.0 },  // bedtime
]);
assert_eq!(sched.target_for_hour(10), 22.0);

// Pre-heating: how many hours ahead to start?
let hours = sched.pre_adjust_hours(18.0, 22.0, 2.0); // 2 hours
```

## API Reference

### `TernaryThermostat`

| Method | Description |
|--------|-------------|
| `TernaryThermostat::new(target, hysteresis)` | Create thermostat at target temp with hysteresis band |
| `.with_pid(kp, ki, kd)` | Configure PID gains |
| `.sense(temp, target)` | Read temperature, return `{-1, 0, +1}` with hysteresis |
| `.regulate(current_state, action)` | Apply state transition with hysteresis |
| `.update_pid(temp)` | Full PID update: compute error → PID → ternary quantize → update state |
| `TernaryThermostat::cycle_efficiency(history)` | Fraction of history spent in idle (0) state |

### `Zone`

Fields: `id: usize`, `target: f64`, `current: f64`, `state: i8`

### `MultiZone`

| Method | Description |
|--------|-------------|
| `MultiZone::new(zones, coupling)` | Create multi-zone system with heat transfer coefficient |
| `.update(hysteresis)` | Simulate heat transfer, compute new states. Returns `Vec<i8>`. |

### `ThermostatScheduler`

| Method | Description |
|--------|-------------|
| `ThermostatScheduler::new(schedule)` | Create from list of (hour, target) entries |
| `.target_for_hour(hour)` | Look up target for given hour |
| `.pre_adjust_hours(current, next, rate)` | Hours needed to reach next target at given rate |

### `ScheduleEntry`

Fields: `hour: u32`, `target: f64`

## The Deeper Idea

The ternary thermostat is a specific instance of a broader pattern: **bang-bang control** with hysteresis. In control theory, bang-bang controllers switch between extreme states (full heating or full cooling) rather than modulating output. This is optimal for systems with on/off actuators — which is most HVAC hardware.

The PID layer adds intelligence to the bang-bang base. The PID computes a continuous correction, but the ternary quantization ensures the hardware only sees three states. The hysteresis band prevents chattering at the switching boundary. Together, they form a two-level control hierarchy: PID for setpoint tracking, ternary for hardware interface.

Multi-zone coupling models a physical truth: buildings are thermally connected. The coupling coefficient is a crude but effective approximation of Fourier's law — heat flows proportionally to the temperature difference between adjacent zones. This creates emergent behavior: heating zone 0 slightly warms zone 1 through the wall, reducing zone 1's own heating demand. The system reaches equilibrium faster than independent zone control.

## Related Crates

- **`ternary-pid`** — the underlying PID controller, extracted as a standalone library
- **`ternary-scheduler`** — scheduling with the same ternary priority model
- **`ternary-route`** — ternary health-aware routing, analogous to zone failover
