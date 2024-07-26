use std::{ffi::c_void, sync::OnceLock};

static START_TIME: OnceLock<std::time::Instant> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
enum Severity {
    Verbose,
    Info,
    Warning,
    Error,
}

impl From<ash::vk::DebugUtilsMessageSeverityFlagsEXT> for Severity {
    fn from(value: ash::vk::DebugUtilsMessageSeverityFlagsEXT) -> Self {
        match value {
            ash::vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => Self::Verbose,
            ash::vk::DebugUtilsMessageSeverityFlagsEXT::INFO => Self::Info,
            ash::vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => Self::Warning,
            ash::vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => Self::Error,
            _ => unimplemented!(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum MessageType {
    General,
    Validation,
    Performance,
    DeviceAddressBinding,
}

impl From<ash::vk::DebugUtilsMessageTypeFlagsEXT> for MessageType {
    fn from(value: ash::vk::DebugUtilsMessageTypeFlagsEXT) -> Self {
        match value {
            ash::vk::DebugUtilsMessageTypeFlagsEXT::GENERAL => Self::General,
            ash::vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION => Self::Validation,
            ash::vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => Self::Performance,
            ash::vk::DebugUtilsMessageTypeFlagsEXT::DEVICE_ADDRESS_BINDING => {
                Self::DeviceAddressBinding
            }
            _ => unimplemented!(),
        }
    }
}

#[allow(clippy::missing_safety_doc)]
pub unsafe extern "system" fn vulkan_debug_callback(
    severity: ash::vk::DebugUtilsMessageSeverityFlagsEXT,
    msg_type: ash::vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const ash::vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _: *mut c_void,
) -> ash::vk::Bool32 {
    let severity: Severity = severity.into();
    let msg_type: MessageType = msg_type.into();

    let msg = p_callback_data
        .as_ref()
        .and_then(|msg| msg.message_as_c_str().and_then(|s| s.to_str().ok()));

    let elapsed = {
        let start = START_TIME.get_or_init(std::time::Instant::now);
        start.elapsed()
    };

    if let Some(msg) = msg {
        println!(
            "{:.6} [{msg_type:?}] [{severity:?}] {msg}",
            elapsed.as_secs_f64(),
        )
    }

    0
}
