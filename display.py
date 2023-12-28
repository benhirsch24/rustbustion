import time
import urllib.request
import digitalio
from PIL import Image, ImageDraw, ImageFont
import board

from adafruit_rgb_display.rgb import color565
from adafruit_rgb_display import st7789

cs_pin = digitalio.DigitalInOut(board.CE0)
dc_pin = digitalio.DigitalInOut(board.D25)
reset_pin = None
BAUDRATE = 64000000
display = st7789.ST7789(
        board.SPI(),
        cs=cs_pin,
        dc=dc_pin,
        rst=reset_pin,
        baudrate=BAUDRATE,
        x_offset=0,
        y_offset=0,
        )

backlight = digitalio.DigitalInOut(board.D22)
backlight.switch_to_output()
backlight.value = True

buttonA = digitalio.DigitalInOut(board.D23)
buttonB = digitalio.DigitalInOut(board.D24)
buttonA.switch_to_input()
buttonB.switch_to_input()

height = display.width
width = display.height
image = Image.new("RGB", (240, 320))
draw = ImageDraw.Draw(image)
#draw.rectangle((0, 0, 90, 90), outline=0, fill=(255,0,0))
#display.image(image, 0)
padding = -2
top = padding
bottom = height - padding
x = 40
y = 80
font = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 24)

draw.rectangle((0, 0, 240, 320), outline=0, fill=0)

try:
    while True:
        draw.rectangle((0, 80, 240, 160), outline=0, fill=(0, 255, 0))
        f = urllib.request.urlopen("http://127.0.0.1:3000")
        temp = f.read().decode("utf-8")
        draw.text((x, y), "Temp: " + temp, font=font, fill="#FFFFFF")
        coords = "X: " + str(x) + " Y: " + str(y)
        draw.text((0, 0), coords, font=font, fill="#FFFFFF")
        display.image(image, 180)
        draw.rectangle((0, 0, 240, 320), outline=0, fill=0)
        if buttonA.value and not buttonB.value:
            y += 5
        if not buttonA.value and buttonB.value:
            y -= 5
        #time.sleep(0.1)
except Exception as e:
    print('exception', e)

backlight.value = False
