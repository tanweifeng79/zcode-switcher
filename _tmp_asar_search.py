# -*- coding: utf-8 -*-
import sys
sys.stdout.reconfigure(encoding="utf-8")
p = r"D:\zcode\resources\app.asar"
with open(p, "rb") as f:
    data = f.read()

pos = data.find(b"quotaExhausted.nextTime`:")
print("pos", pos)
print(data[pos-2500:pos+800].decode("utf-8", "replace"))

print("\n\n===== function $vt earlier =====")
# find switch around claim failure
pos2 = data.find(b"function $vt")
print("function $vt", pos2)
if pos2 < 0:
    pos2 = data.find(b"$vt=s(")
    print("$vt=s", pos2)
pos3 = data.find(b"claim.failure.alreadyClaimed")
print("alreadyClaimed code mapping?", pos3)
# search JS mapping not i18n - look for case 13
idx = 0
while True:
    i = data.find(b"case 1304", idx)
    if i < 0:
        break
    if 297900000 < i < 298050000:
        print("case 1304 at", i)
        print(data[i-400:i+800].decode("utf-8","replace"))
        break
    idx = i+1
