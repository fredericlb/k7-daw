// performance.ts — how capable an engine's MACHINE is, as ONE integer, in the shape the heartbeat carries.
// The supervisor measures and scores it (k7-supervisor's performance.rs); this file only types what
// the page reads. `null` is "could not measure", never "slow". See docs/performance.md.

/** What the supervisor measures. */
export interface EnginePerformanceMetrics {
  /** Logical cores the OS offers this process. */
  cores: number;
  /** One core's throughput on the reference DSP loop, in millions of samples per second. */
  singleThread: number;
  /** Sustained write to the state dir's disk, fsync included, MB/s. */
  diskWriteMBps: number | null;
  /** Small durable writes per second — what a take being recorded in blocks feels like. */
  fsyncPerSec: number | null;
  /** Physical memory, GiB. */
  memoryGB: number;
}

export interface EnginePerformance extends EnginePerformanceMetrics {
  /** The rating: 1000 is the reference machine, higher is faster. */
  score: number;
}
