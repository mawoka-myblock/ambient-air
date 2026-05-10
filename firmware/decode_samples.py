import struct
from dataclasses import dataclass
from typing import List

# Matches Rust #[repr(C, packed)] struct exactly
# < = little-endian
# i = int32
# I = uint32
# H = uint16
# h = int16
#
# Layout:
# temp_p   i32
# pressure u32
# temp_t   i32
# humidity u16
# co2      i16
# voc      i32
#
# Total = 22 bytes
MEASUREMENT_STRUCT = struct.Struct("<i I i H h i")

DATA = """
B3-08-00-00-EC-88-01-00-B3-08-00-00-32-00-59-0B-00-00-00-00-B3-08-00-00-EC-88-01-00-B3-08-00-00-32-00-A6-0A-00-00-00-00-B3-08-00-00-ED-88-01-00-B3-08-00-00-31-00-10-0A-00-00-00-00-B4-08-00-00-EC-88-01-00-B4-08-00-00-31-00-97-09-00-00-00-00-B4-08-00-00-EC-88-01-00-B4-08-00-00-31-00-0A-09-00-00-00-00-B4-08-00-00-F1-88-01-00-B4-08-00-00-31-00-DB-08-00-00-00-00-B6-08-00-00-F0-88-01-00-B6-08-00-00-31-00-CD-08-00-00-00-00-B6-08-00-00-EE-88-01-00-B6-08-00-00-31-00-92-08-00-00-00-00-B8-08-00-00-F0-88-01-00-B8-08-00-00-30-00-CA-08-00-00-00-00-B9-08-00-00-ED-88-01-00-B9-08-00-00-30-00-C3-08-00-00-00-00-B8-08-00-00-EA-88-01-00-B8-08-00-00-30-00-BA-08-00-00-00-00-B8-08-00-00-EB-88-01-00-B8-08-00-00-30-00-A3-08-00-00-00-00-BA-08-00-00-EC-88-01-00-BA-08-00-00-30-00-8A-08-00-00-00-00-B8-08-00-00-F0-88-01-00-B8-08-00-00-30-00-88-08-00-00-00-00-B9-08-00-00-F1-88-01-00-B9-08-00-00-30-00-78-08-00-00-00-00-BA-08-00-00-F4-88-01-00-BA-08-00-00-30-00-90-08-00-00-00-00-BA-08-00-00-F2-88-01-00-BA-08-00-00-30-00-A2-08-00-00-00-00-BA-08-00-00-EE-88-01-00-BA-08-00-00-30-00-98-08-00-00-00-00-B9-08-00-00-EE-88-01-00-B9-08-00-00-30-00-9E-08-00-00-00-00-BA-08-00-00-EE-88-01-00-BA-08-00-00-30-00-96-08-00-00-00-00-B8-08-00-00-F2-88-01-00-B8-08-00-00-30-00-93-08-00-00-00-00-BA-08-00-00-F1-88-01-00-BA-08-00-00-30-00-A2-08-00-00-00-00-B9-08-00-00-F1-88-01-00-B9-08-00-00-2F-00-A2-08-00-00-00-00-BC-08-00-00-EE-88-01-00-BC-08-00-00-2F-00-A3-08-00-00-00-00-BD-08-00-00-ED-88-01-00-BD-08-00-00-2F-00-8D-08-00-00-00-00
"""


@dataclass
class Measurement:
    temp_p: float
    pressure: float
    temp_t: float
    humidity: int
    co2: int
    voc: int


def decode_measurements(data: bytes) -> List[Measurement]:
    """Decode a byte array into a list of Measurement objects."""

    if len(data) % MEASUREMENT_STRUCT.size != 0:
        raise ValueError(
            f"Data length {len(data)} is not a multiple of {MEASUREMENT_STRUCT.size}"
        )

    measurements = []

    for offset in range(0, len(data), MEASUREMENT_STRUCT.size):
        raw = MEASUREMENT_STRUCT.unpack_from(data, offset)

        temp_p_raw, pressure_raw, temp_t_raw, humidity, co2, voc = raw

        measurements.append(
            Measurement(
                temp_p=temp_p_raw / 100.0,  # 2623 -> 26.23 °C
                pressure=pressure_raw / 1000.0,  # 12345 -> 12.345 Pa
                temp_t=temp_t_raw / 100.0,  # 2623 -> 26.23 °C
                humidity=humidity,  # already %
                co2=co2,  # ppm
                voc=voc,  # index
            )
        )

    return measurements


def parse_dash_hex(hex_string: str) -> bytes:
    """

    Convert 'aa-bb-cc-dd' hex string into raw bytes.

    """

    hex_string = hex_string.strip().replace(" ", "")

    return bytes(int(b, 16) for b in hex_string.split("-"))


# Example usage:
if __name__ == "__main__":
    # Example dummy buffer (replace with your actual data)

    decoded = decode_measurements(parse_dash_hex(DATA))

    for i, m in enumerate(decoded):
        print(f"Measurement #{i}: {m}")
