
def generate_byte():
    start = "S"
    first = "hello"
    first += (63-len(first))*"0"
    print(first)
    second = "world"
    second += (60-len(second))*"0"
    print(second)
    final = "0000"
    print(start+first+second+final)

generate_byte()
