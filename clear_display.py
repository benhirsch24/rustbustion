import time
import urllib.request
import digitalio
from PIL import Image, ImageDraw, ImageFont
import board
import json

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
backlight.value = False
