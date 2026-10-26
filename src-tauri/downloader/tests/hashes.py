import hashlib

data = bytes((i * 31 + 7) % 256 for i in range(1_000_000))

print("Length:", len(data))
print("SHA-256:", hashlib.sha256(data).hexdigest())
print("SHA-512:", hashlib.sha512(data).hexdigest())
print("MD5:    ", hashlib.md5(data).hexdigest())
