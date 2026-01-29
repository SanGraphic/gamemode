use windows::Win32::System::Power::{
    PowerSetActiveScheme, PowerGetActiveScheme, PowerWriteACValueIndex, PowerReadACValueIndex,
};
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::core::GUID;
use std::ptr;
use std::process::Command;
use std::os::windows::process::CommandExt;

// ============================================================================
// GUIDs from PowerService.cs
// ============================================================================

// 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c (High Performance)
const GUID_HIGH_PERFORMANCE: GUID = GUID::from_u128(0x8c5e7fda_e8bf_4a96_9a85_a6e23a8c635c);

// e9a42b02-d5df-448d-aa00-03f14749eb61 (Ultimate Performance)
const GUID_ULTIMATE_PERFORMANCE: GUID = GUID::from_u128(0xe9a42b02_d5df_448d_aa00_03f14749eb61);

// 54533251-82be-4824-96c1-47b60b740d00 (Processor Subgroup)
// C#: private static Guid PROCESSOR_SUBGROUP = new Guid("54533251-82be-4824-96c1-47b60b740d00");
const GUID_PROCESSOR_SUBGROUP: GUID = GUID::from_u128(0x54533251_82be_4824_96c1_47b60b740d00);

// be337238-0d82-4146-a960-4f3749d470c7 (Perf Boost Mode)
// C#: private static Guid PERF_BOOST_MODE = new Guid("be337238-0d82-4146-a960-4f3749d470c7");
const GUID_PROCESSOR_PERF_BOOST_MODE: GUID = GUID::from_u128(0xbe337238_0d82_4146_a960_4f3749d470c7);

// 893dee8e-2bef-41e0-89c6-b55d0929964c (Min Processor State)
// C#: private static Guid MIN_PROCESSOR_STATE = new Guid("893dee8e-2bef-41e0-89c6-b55d0929964c");
const GUID_PROCESSOR_THROTTLE_MINIMUM: GUID = GUID::from_u128(0x893dee8e_2bef_41e0_89c6_b55d0929964c);

/// PowerService - 1:1 port of PowerService.cs
/// Handles power plan switching for both desktop and laptop scenarios
pub struct PowerService {
    // Original power scheme GUID to restore later (1:1 with C# _originalScheme)
    original_scheme: Option<GUID>,
    // For laptop: original boost mode value (1:1 with C# _originalBoostMode)
    original_boost_mode: Option<u32>,
    // For laptop: original min processor state (1:1 with C# _originalMinProcessor)
    original_min_processor: Option<u32>,
    // For laptop: the active scheme when we modified it
    laptop_active_scheme: Option<GUID>,
}

impl PowerService {
    pub fn new() -> Self {
        // Get and store current active scheme at startup
        let original_scheme = unsafe {
            let mut scheme_ptr = ptr::null_mut();
            if PowerGetActiveScheme(None, &mut scheme_ptr).is_ok() && !scheme_ptr.is_null() {
                let scheme = *scheme_ptr;
                let _ = LocalFree(HLOCAL(scheme_ptr as *mut _));
                Some(scheme)
            } else {
                None
            }
        };

        Self {
            original_scheme,
            original_boost_mode: None,
            original_min_processor: None,
            laptop_active_scheme: None,
        }
    }

    /// 1:1 port of SetHighPerformance() from PowerService.cs
    /// Used for DESKTOP systems
    /// Logic: Try Ultimate Performance, if not found duplicate High Performance, else use High Performance
    pub fn set_high_performance(&mut self) {
        unsafe {
            // Store original scheme for revert
            let mut scheme_ptr = ptr::null_mut();
            if PowerGetActiveScheme(None, &mut scheme_ptr).is_ok() && !scheme_ptr.is_null() {
                self.original_scheme = Some(*scheme_ptr);
                let _ = LocalFree(HLOCAL(scheme_ptr as *mut _));
            }

            // Check if Ultimate Performance exists using powercfg
            // C#: this.PowerPlanExists(GUID_ULTIMATE_PERFORMANCE)
            let ultimate_exists = self.power_plan_exists(&GUID_ULTIMATE_PERFORMANCE);
            
            if ultimate_exists {
                // Activate Ultimate Performance
                if PowerSetActiveScheme(None, Some(&GUID_ULTIMATE_PERFORMANCE)).is_err() {
                    // Fall back to High Performance
                    let _ = PowerSetActiveScheme(None, Some(&GUID_HIGH_PERFORMANCE));
                }
            } else {
                // C#: Try to duplicate the scheme to create it
                // this.DuplicatePowerScheme(GUID_ULTIMATE_PERFORMANCE);
                self.duplicate_power_scheme(&GUID_ULTIMATE_PERFORMANCE);
                
                // Check again
                let ultimate_exists_now = self.power_plan_exists(&GUID_ULTIMATE_PERFORMANCE);
                
                if ultimate_exists_now {
                    if PowerSetActiveScheme(None, Some(&GUID_ULTIMATE_PERFORMANCE)).is_err() {
                        let _ = PowerSetActiveScheme(None, Some(&GUID_HIGH_PERFORMANCE));
                    }
                } else {
                    // Fall back to High Performance
                    let _ = PowerSetActiveScheme(None, Some(&GUID_HIGH_PERFORMANCE));
                }
            }
        }
    }

    /// 1:1 port of OptimizeLaptopBoost() from PowerService.cs
    /// Used for LAPTOP systems
    /// Modifies current scheme's processor boost mode and min processor state
    pub fn optimize_laptop_boost(&mut self) {
        unsafe {
            // Get current active scheme
            let mut scheme_ptr = ptr::null_mut();
            if PowerGetActiveScheme(None, &mut scheme_ptr).is_err() || scheme_ptr.is_null() {
                return;
            }
            let active_scheme = *scheme_ptr;
            self.laptop_active_scheme = Some(active_scheme);
            let _ = LocalFree(HLOCAL(scheme_ptr as *mut _));

            // Read and store original boost mode value
            // C#: PowerReadACValueIndex(IntPtr.Zero, ref scheme, ref PROCESSOR_SUBGROUP, ref PERF_BOOST_MODE, out originalBoost);
            let mut current_boost: u32 = 0;
            if PowerReadACValueIndex(
                None,
                Some(&active_scheme as *const GUID),
                Some(&GUID_PROCESSOR_SUBGROUP),
                Some(&GUID_PROCESSOR_PERF_BOOST_MODE),
                &mut current_boost
            ).is_ok() {
                self.original_boost_mode = Some(current_boost);
            }

            // Set boost mode to 4 (Aggressive)
            // C#: PowerWriteACValueIndex(IntPtr.Zero, ref scheme, ref PROCESSOR_SUBGROUP, ref PERF_BOOST_MODE, 4);
            let _ = PowerWriteACValueIndex(
                None,
                &active_scheme,
                Some(&GUID_PROCESSOR_SUBGROUP),
                Some(&GUID_PROCESSOR_PERF_BOOST_MODE),
                4 // Aggressive
            );

            // Read and store original min processor state
            let mut current_min: u32 = 0;
            if PowerReadACValueIndex(
                None,
                Some(&active_scheme as *const GUID),
                Some(&GUID_PROCESSOR_SUBGROUP),
                Some(&GUID_PROCESSOR_THROTTLE_MINIMUM),
                &mut current_min
            ).is_ok() {
                self.original_min_processor = Some(current_min);
            }

            // Set min processor state to 100%
            // C#: PowerWriteACValueIndex(IntPtr.Zero, ref scheme, ref PROCESSOR_SUBGROUP, ref MIN_PROCESSOR_STATE, 100);
            let _ = PowerWriteACValueIndex(
                None,
                &active_scheme,
                Some(&GUID_PROCESSOR_SUBGROUP),
                Some(&GUID_PROCESSOR_THROTTLE_MINIMUM),
                100
            );

            // Re-apply scheme to take effect
            // C#: PowerSetActiveScheme(IntPtr.Zero, ref scheme);
            let _ = PowerSetActiveScheme(None, Some(&active_scheme));
        }
    }

    /// 1:1 port of RevertPowerPlan() from PowerService.cs
    /// Used for DESKTOP systems to restore original power plan
    pub fn revert_power_plan(&self) {
        unsafe {
            if let Some(original) = self.original_scheme {
                let _ = PowerSetActiveScheme(None, Some(&original));
            }
        }
    }

    /// 1:1 port of RevertLaptopBoost() from PowerService.cs
    /// Used for LAPTOP systems to restore original boost mode and min processor state
    pub fn revert_laptop_boost(&self) {
        unsafe {
            if let Some(scheme) = self.laptop_active_scheme {
                // Restore original boost mode
                if let Some(original_boost) = self.original_boost_mode {
                    let _ = PowerWriteACValueIndex(
                        None,
                        &scheme,
                        Some(&GUID_PROCESSOR_SUBGROUP),
                        Some(&GUID_PROCESSOR_PERF_BOOST_MODE),
                        original_boost
                    );
                }

                // Restore original min processor state
                if let Some(original_min) = self.original_min_processor {
                    let _ = PowerWriteACValueIndex(
                        None,
                        &scheme,
                        Some(&GUID_PROCESSOR_SUBGROUP),
                        Some(&GUID_PROCESSOR_THROTTLE_MINIMUM),
                        original_min
                    );
                }

                // Re-apply to take effect
                let _ = PowerSetActiveScheme(None, Some(&scheme));
            }
        }
    }

    /// Generic revert that calls the appropriate method based on system type
    /// (Kept for backwards compatibility)
    #[allow(dead_code)]
    pub fn revert(&self) {
        // This is called from places that don't know if it's desktop or laptop
        // Just restore original scheme which works for both cases
        self.revert_power_plan();
    }

    /// 1:1 port of PowerPlanExists() from PowerService.cs
    /// Checks if a power plan GUID exists using powercfg /list
    fn power_plan_exists(&self, guid: &GUID) -> bool {
        let output = Command::new("powercfg")
            .args(["/list"])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output();

        if let Ok(o) = output {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let guid_str = format!("{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                guid.data1, guid.data2, guid.data3,
                guid.data4[0], guid.data4[1],
                guid.data4[2], guid.data4[3], guid.data4[4], guid.data4[5], guid.data4[6], guid.data4[7]
            );
            return stdout.to_lowercase().contains(&guid_str.to_lowercase());
        }
        false
    }

    /// 1:1 port of DuplicatePowerScheme() from PowerService.cs
    /// Duplicates a power scheme using powercfg -duplicatescheme
    fn duplicate_power_scheme(&self, guid: &GUID) {
        let guid_str = format!("{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            guid.data1, guid.data2, guid.data3,
            guid.data4[0], guid.data4[1],
            guid.data4[2], guid.data4[3], guid.data4[4], guid.data4[5], guid.data4[6], guid.data4[7]
        );

        let _ = Command::new("powercfg")
            .args(["-duplicatescheme", &guid_str])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output();
    }
}
