
def generate_byte():
    first = "hello"
    first += (63-len(first))*"0"
    print(first)
    second = "world"
    second += (60-len(second))*"0"
    print(second)
    final = "0000"
    print(first+second+final)
    first = "hello"
    first += (31-len(first))*"0"
    print(first)
    second = "world"
    second += (28-len(second))*"0"
    print(second)
    final = "0000"
    print(first+second+final)

generate_byte()
