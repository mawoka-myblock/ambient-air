use crate::{COMMAND_CHANNEL, CONFIG_SIGNAL, Commands};
use defmt::unwrap;

#[embassy_executor::task]
pub async fn settings_task() {
    let mut cmd_listener = unwrap!(COMMAND_CHANNEL.subscriber());
    let cfg_signal = CONFIG_SIGNAL.sender();
    loop {
        if let Commands::Reconfigure(d) = cmd_listener.next_message_pure().await {
            cfg_signal.send(d);
        }
    }
}
