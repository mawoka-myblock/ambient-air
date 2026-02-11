# Bluetooth LE Docs for Ambient-Air

- Every 2nd level headline is a service
- Every 3rd level headline is a characteristic
- Headline is as follows structured: `Name (UUID) (rust data type) access type (R=Read, W=Write, N=Notify)`

## BaseData (1aba5096-5be2-4768-aef0-51c8667e1aa8)
No content, used to identify devices compatible with Mawoka App

## Battery Service (0x180F)

### Battery Level (0x2a19) (u8) R N
Battery percentage 0-100

### Battery Power (408813df-5dd4-1f87-ec11-cdb001100000) (i16) R N
Battery average power in milliwatt

### Battery Capacity (10bb24ab-a674-4630-b670-972eac8bf6cb) (i16) R W
Design battery capacity in mAh, used for battery percentage calculation

## Temperature Service (125ef8ff-f538-468f-9f40-2380a102895b)
Data provided by AHT20

### Temperature (561be71a-359d-4964-b64f-7b1c949b092e) (f32) R N
Temperature in °C

### Humidity (13881d03-54b9-4b8c-be9f-8a0eeec6893b) (f32) R N
Relative Humidity in % (eg. 50.24%)

## Pressure Service (5f78b426-c2dd-4c3f-864f-1b2ccdf1e63e)
Data by ICP-20100 (also temperature reading from this sensor)

### Pressure (7c4b9d53-cbce-409e-bb3d-06d7f9f263d8) (f32) R N
Air pressure in kPa

### Temperature (a3f6145d-d2eb-46a6-aa41-9644a44bb18e) (f32) R N
Temperature in °C

## Co2 Service (a6689992-6e99-4903-85ce-5750b7c4d995)
Data by STCC4

### Co2 (cfb04cf1-8d5b-4223-9ae5-c9e32b2940ab) (i16) R N
Co2 contentration in ppm

### Sampling Intverval (22b0808a-3a60-45ed-9c54-57f1f16079e6) (i16) R W
Sampling rate in seconds

This sensor needs to be sampled actively to get accurate readings.
The sampling should happen between every 5 seconds and every 600 seconds (10 minutes). This applies only to the standby operation. During normal operation, the sensor gets sampled every 5 seconds.

## Voc Service (9fdbefc6-0e57-469c-b006-8c38f517805a)

### Index (55697045-6e90-4940-b055-a03f9ae10122) (i16) R N
The VOC index according to Sensirion. Reads 0 when the sensor isn't ready
or deactivated

### Count (93c32824-5d3b-4343-a5e9-5699d165bc47) (i16) R N
The readings still needed until data is provided (60 from boot).

### Enabled (a1666baa-2fd2-456b-ab68-8e83395f9f79) (bool) R W
Enable or disable the sensor. This needs to be sampled every seconds and consumes lots of power and messes with temperature readings. Global setting, affects the Sampling Programs as well.

## Measurement Service (aa830336-a632-4fb7-83c0-c3868760d858)
This service provides access to sampling programs.

### Command (84b0a39c-c55f-41f3-8797-de87992adc55) (String) W
This JSON structure as UTF-8, finished with an `\0`
```JSON
{
    "every_x_seconds": "number",
    "samples": "number"
}
```
Writing this immediately starts the sampling every x seconds and takes x samples.

### Data (127ec103-86ea-4e75-9e35-2e0c772d6f85) (&[u8]) R
Reads sampling data as an array of C-like structs of following structure:
```rust
#[repr(C, packed)]
pub struct Measurement {
    temp_p: i32,   // 2623 -> 26.23°C
    pressure: u32, // 12345 -> 12.345 Pa
    temp_t: i32,   // 2623 -> 26.23°C
    humidity: u16, // 42 -> 42%
    co2: i16, // in ppm
    voc: i32, // VOC index according to Sensirion
}
assert_eq!(size_of::<Measurement>(), 22);
```
So each entry is 22 bytes for a max total of 3520 bytes/reading

## Time Service (85083006-8da2-4d0b-9dca-fc3ccda46a3c)

### Time (u64) R W
The current time in **µs**
