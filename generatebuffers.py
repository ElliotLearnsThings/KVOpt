
import uuid
import struct
from datetime import datetime

def generate_buffers(command: bytes, num_buffers: int) -> list[bytearray]:
    buffers: list[bytearray] = []

    for _ in range(num_buffers):
        iuuid = uuid.uuid4().bytes  # UUID as bytes
        ibreak = b"0" * (64 - 13)  # Padding with zero bytes
        fuuid = uuid.uuid4().bytes  # Another UUID as bytes
        fbreak = b"0" * (56 - 20)  # Padding with zero bytes

        # Current timestamp (in seconds)
        tcur = int(datetime.now().timestamp())
        tcurb = struct.pack(">Q", tcur)  # Pack timestamp as 8 bytes in big-endian format
        tcurb = tcurb[2:]  # Take last 6 bytes of the timestamp (big-endian)

        # Expiry time, 10 seconds as i16
        tlen = struct.pack(">h", 10)

        # Combine the timestamp and expiry time into a buffer of length 8
        tbuf = tcurb + tlen

        # Combine all parts into one byte buffer
        buffer_bytes = command + iuuid + ibreak + fuuid + fbreak + tbuf

        # Limit the size to 128 bytes
        byte_array = bytearray(128)
        byte_array[:len(buffer_bytes)] = buffer_bytes[:128]  # Ensure it doesn't exceed 128 bytes
        buffers.append(byte_array)

    return buffers

def save_buffers_to_file(buffers: list[bytearray], filename: str = "testcachevals.env"):
    with open(filename, "w") as f:  # Open in text mode to write hex string
        for buffer in buffers:
            # Convert each byte in the bytearray to the hex format \x00
            hex_str = " ".join(f"{byte:02x}" for byte in buffer)
            # Write the hex string to the file, followed by a newline
            _ = f.write(hex_str + "\n")
        print(f"Data saved to {filename}")


# Generate buffers
buffers = generate_buffers(b"I", 1)
buffers.append(generate_buffers(b"G", 1)[0])
buffers.append(generate_buffers(b"R", 1)[0])

# Save to file
save_buffers_to_file(buffers)

