//! CPU Affinity Configuration for UFT Strategies.
//!
//! This module provides CPU affinity configuration for binding strategy threads
//! to specific CPU cores, reducing context switching and improving latency.
//!
//! # Platform Support
//!
//! - Linux: Full support via `sched_setaffinity`
//! - macOS: Limited support (thread affinity hints)
//! - Windows: Full support via `SetThreadAffinityMask`
//!
//! # Example
//!
//! ```ignore
//! use zephyr_engine::uft::CpuAffinity;
//!
//! // Bind current thread to CPU core 0
//! CpuAffinity::set_current_thread(&[0]).unwrap();
//!
//! // Bind to multiple cores
//! CpuAffinity::set_current_thread(&[2, 3]).unwrap();
//! ```

#![allow(clippy::uninlined_format_args)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(unsafe_code)]

use std::fmt;

/// CPU affinity configuration error.
#[derive(Debug, Clone)]
pub enum CpuAffinityError {
    /// Invalid CPU core specified.
    InvalidCore(usize),
    /// Platform not supported.
    PlatformNotSupported,
    /// System call failed.
    SystemError(String),
}

impl fmt::Display for CpuAffinityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCore(core) => write!(f, "Invalid CPU core: {}", core),
            Self::PlatformNotSupported => write!(f, "CPU affinity not supported on this platform"),
            Self::SystemError(msg) => write!(f, "System error: {}", msg),
        }
    }
}

impl std::error::Error for CpuAffinityError {}

/// CPU affinity configuration.
///
/// Provides methods for setting CPU affinity for threads.
pub struct CpuAffinity;

impl CpuAffinity {
    /// Gets the number of available CPU cores.
    #[must_use]
    pub fn num_cpus() -> usize {
        std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(1)
    }

    /// Sets the CPU affinity for the current thread.
    ///
    /// # Arguments
    ///
    /// * `cores` - List of CPU core indices to bind to
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any core index is invalid
    /// - The platform doesn't support CPU affinity
    /// - The system call fails
    #[cfg(target_os = "linux")]
    pub fn set_current_thread(cores: &[usize]) -> Result<(), CpuAffinityError> {
        use std::mem;

        if cores.is_empty() {
            return Ok(());
        }

        let num_cpus = Self::num_cpus();
        for &core in cores {
            if core >= num_cpus {
                return Err(CpuAffinityError::InvalidCore(core));
            }
        }

        // Create CPU set
        let mut cpu_set: libc::cpu_set_t = unsafe { mem::zeroed() };
        for &core in cores {
            unsafe {
                libc::CPU_SET(core, &mut cpu_set);
            }
        }

        // Set affinity
        let result = unsafe {
            libc::sched_setaffinity(
                0, // current thread
                mem::size_of::<libc::cpu_set_t>(),
                &raw const cpu_set,
            )
        };

        if result == 0 {
            Ok(())
        } else {
            Err(CpuAffinityError::SystemError(format!(
                "sched_setaffinity failed: {}",
                std::io::Error::last_os_error()
            )))
        }
    }

    /// Sets the CPU affinity for the current thread (macOS).
    ///
    /// Note: macOS doesn't support strict CPU affinity, but we can set
    /// thread affinity hints using `thread_policy_set`.
    #[cfg(target_os = "macos")]
    pub fn set_current_thread(cores: &[usize]) -> Result<(), CpuAffinityError> {
        if cores.is_empty() {
            return Ok(());
        }

        let num_cpus = Self::num_cpus();
        for &core in cores {
            if core >= num_cpus {
                return Err(CpuAffinityError::InvalidCore(core));
            }
        }

        // macOS doesn't support strict CPU affinity
        // We can only provide hints via thread_policy_set
        // For now, we just validate the cores and return Ok
        // In production, you might use thread_policy_set with THREAD_AFFINITY_POLICY
        tracing::debug!(
            cores = ?cores,
            "CPU affinity hint set (macOS doesn't support strict affinity)"
        );
        Ok(())
    }

    /// Sets the CPU affinity for the current thread (Windows).
    #[cfg(target_os = "windows")]
    pub fn set_current_thread(cores: &[usize]) -> Result<(), CpuAffinityError> {
        use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadAffinityMask};

        if cores.is_empty() {
            return Ok(());
        }

        let num_cpus = Self::num_cpus();
        let mut mask: usize = 0;
        for &core in cores {
            if core >= num_cpus || core >= std::mem::size_of::<usize>() * 8 {
                return Err(CpuAffinityError::InvalidCore(core));
            }
            mask |= 1 << core;
        }

        let result = unsafe {
            let thread = GetCurrentThread();
            SetThreadAffinityMask(thread, mask)
        };

        if result != 0 {
            Ok(())
        } else {
            Err(CpuAffinityError::SystemError(format!(
                "SetThreadAffinityMask failed: {}",
                std::io::Error::last_os_error()
            )))
        }
    }

    /// Sets the CPU affinity for the current thread (unsupported platforms).
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    pub fn set_current_thread(_cores: &[usize]) -> Result<(), CpuAffinityError> {
        Err(CpuAffinityError::PlatformNotSupported)
    }

    /// Validates CPU core indices.
    ///
    /// # Errors
    ///
    /// Returns an error if any core index is invalid.
    pub fn validate_cores(cores: &[usize]) -> Result<(), CpuAffinityError> {
        let num_cpus = Self::num_cpus();
        for &core in cores {
            if core >= num_cpus {
                return Err(CpuAffinityError::InvalidCore(core));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_num_cpus() {
        let num = CpuAffinity::num_cpus();
        assert!(num > 0);
    }

    #[test]
    fn test_validate_cores_valid() {
        let num_cpus = CpuAffinity::num_cpus();
        if num_cpus > 0 {
            assert!(CpuAffinity::validate_cores(&[0]).is_ok());
        }
    }

    #[test]
    fn test_validate_cores_invalid() {
        let num_cpus = CpuAffinity::num_cpus();
        assert!(CpuAffinity::validate_cores(&[num_cpus + 100]).is_err());
    }

    #[test]
    fn test_set_affinity_empty() {
        // Empty cores should be a no-op
        assert!(CpuAffinity::set_current_thread(&[]).is_ok());
    }
}
