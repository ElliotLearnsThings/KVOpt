
import subprocess

# Read the raw byte data from the file
with open("testcachevals.env") as f:
    data = f.readlines()

# Remove the '\x' and then decode the hex values
data = [bytes.fromhex(x.strip()) for x in data]

# Print the original data (the raw string from the file)
for x in data:
    print(x)

# Command to run your Rust program
rust_program = '/Users/elliothegraeus/Documents/BASE/projects/cacherebook/target/release/cacherebbok'  # Ensure the path is correct

# Start the Rust program and pipe the bytes into its stdin
process = subprocess.Popen(
    rust_program, 
    stdin=subprocess.PIPE, 
    stdout=subprocess.PIPE, 
    stderr=subprocess.PIPE
)



# Wait for the process to finish
stdout, stderr = process.communicate(input=b"G")

# Print the output from the Rust program (if any)
print("STDOUT:", stdout.decode())
print("STDERR:", stderr.decode())


