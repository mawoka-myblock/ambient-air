use crate::data::Devices;

pub async fn deep_sleep<'a>(devices: &mut Devices<'a>) {
    devices.sgp40.lock().await.turn_heater_off().await.unwrap(); // 34µA
    // AHT20 standby current of 0.25µA
    // ICP-20100 has to be set to MODE 3 for 23µA current
    devices.stcc4.lock().await.enter_sleep_mode().await.unwrap(); // 1µA
    // devices.bq27441.enter_hibernate().await.unwrap() //
}
