# ternary-thermostat

Ternary climate control with **PID compensation**, **multi-zone heat transfer**, and **time-based scheduling**. The thermostat output is ternary: `+1` (heating), `0` (idle), `-1` (cooling), enabling precise bang-bang control with hysteresis and PID fine-tuning.

## Why It Matters

Binary thermostats (on/off) cause rapid cycling near the setpoint, wearing out HVAC components and creating temperature oscillation. Ternary control adds an explicit **idle** state with hysteresis deadband, ensuring the system only activates when the temperature diverges meaningfully:

| State | Value | Action |
|-------|-------|--------|
| Heating | `+1` | Activate heater |
| Idle | `0` | No action (within deadband) |
| Cooling | `-1` | Activate cooler |

The multi-zone model adds spatial awareness: adjacent zones exchange heat proportional to a coupling coefficient, modeling real-world thermal conductivity between rooms.

## How It Works

### Bang-Bang Control with Hysteresis

The basic control law:

```
diff = target - current_temp

if diff > hysteresis:    state = +1  (heat)
if diff < -hysteresis:   state = -1  (cool)
otherwise:               state =  0  (idle)
```

The hysteresis band `[-h, +h]` prevents rapid switching. A typical value of `h = 0.5°C` means the system won't activate until temperature drifts half a degree from setpoint.

**Complexity:** O(1) per evaluation.

### PID Controller

For finer control, a discrete PID controller computes:

```
u(t) = Kp · e(t) + Ki · Σe(τ)dτ + Kd · de/dt
```

where:
- `e(t) = target - measured` (error)
- `Kp` = proportional gain
- `Ki` = integral gain (with anti-windup clamping to ±100)
- `Kd` = derivative gain

The PID output is then thresholded to ternary:

```
if u > hysteresis:  state = +1
if u < -hysteresis: state = -1
otherwise:          state = 0
```

**Anti-windup:** The integral term is clamped to [-100, 100] to prevent integral saturation during sustained errors.

**Complexity:** O(1) per PID update.

### Multi-Zone Thermal Model

Adjacent zones exchange heat via a coupling coefficient κ:

```
T'_i = T_i + κ · (T_{i-1} - T_i) + κ · (T_{i+1} - T_i)
```

This is a **1D heat diffusion** model — the discrete Laplacian with coupling κ as the diffusion coefficient. After updating temperatures, each zone's ternary state is recomputed.

**Complexity:** O(Z) per update, where Z = zone count.

**Stability:** The explicit Euler scheme is stable when `κ < 0.5` (CFL condition for 1D diffusion).

### Scheduling and Pre-Adjustment

Time-based schedules adjust targets by hour:

```
target(hour) = last_entry where entry.hour ≤ hour
```

Pre-adjustment calculates lead time:

```
pre_hours = |next_target - current_target| / rate_per_hour
```

This enables pre-heating or pre-cooling before a scheduled setpoint change, ensuring the zone reaches target by the scheduled time.

## Quick Start

```rust
use ternary_thermostat::{TernaryThermostat, MultiZone, Zone, ThermostatScheduler, ScheduleEntry};

// Single-zone with PID
let mut tstat = TernaryThermostat::new(target: 22.0, hysteresis: 0.5)
    .with_pid(kp: 1.0, ki: 0.1, kd: 0.5);

let state = tstat.update_pid(18.0);  // cold room
assert_eq!(state, 1);  // heating

// Multi-zone
let zones = vec![
    Zone { id: 0, target: 22.0, current: 18.0, state: 0 },
    Zone { id: 1, target: 22.0, current: 26.0, state: 0 },
];
let mut mz = MultiZone::new(zones, coupling: 0.1);
let states = mz.update(0.5);
assert_eq!(states, vec![1, -1]);  // heat cold zone, cool hot zone
```

## API

| Type | Key Methods |
|------|-------------|
| `TernaryThermostat` | `new(target, hysteresis)`, `with_pid(kp,ki,kd)`, `sense(temp, target)`, `regulate(current, action)`, `update_pid(temp)` |
| `MultiZone` | `new(zones, coupling)`, `update(hysteresis)` → `Vec<i8>` |
| `ThermostatScheduler` | `new(schedule)`, `target_for_hour(h)`, `pre_adjust_hours(current, next, rate)` |
| `Zone` | `{ id, target, current, state }` |
| `ScheduleEntry` | `{ hour, target }` |

## Architecture Notes

The **γ + η = C** invariant: *generation* (γ) is the heating/cooling action producing temperature changes, *entropy* (η) is the temperature differential distribution across zones (high entropy = large gradients), and *conservation* (C) is the **first law of thermodynamics** — energy is conserved as heat flows between zones. The multi-zone coupling term `κ(T_{i±1} - T_i)` is the discrete form of Fourier's law of heat conduction, enforcing energy conservation. The cycle_efficiency metric (fraction of time in idle state) measures how well γ and η are balanced: high efficiency means the system is in equilibrium (η ≈ 0), requiring minimal corrective action (γ ≈ 0).

## References

- **PID control:** Åström, K. & Hägglund, T. *Advanced PID Control* (2006)
- **Hysteresis in control systems:** Slotine, J.-J. & Li, W. *Applied Nonlinear Control* (1991), §2.3
- **Heat diffusion:** Carslaw, H. & Jaeger, J. *Conduction of Heat in Solids* (1959)
- **Smart thermostat scheduling:** Lu, J. et al. "The Smart Thermostat" (2010)

## License

MIT
