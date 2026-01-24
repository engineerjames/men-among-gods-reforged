# How to generate .ico file from .png for Windows icon support
> Requires ImageMagick to be installed
```
convert client/assets/gfx/mag_logo.png -define icon:auto-resize=256,64,48,32,16 client/assets/gfx/mag_logo.ico
```