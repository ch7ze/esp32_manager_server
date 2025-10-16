import serial
import time

ser = serial.Serial('COM14', 115200)
time.sleep(0.1)

# JSON Nachricht senden
for i in range(500):
    for j in range(5):
        msg = f'{{"device_id": "esp32-0{j}", "temperature": "25.5"}}\n\r'
        ser.write(msg.encode())
        print(msg)
    time.sleep(5)

ser.close()
