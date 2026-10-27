#!/bin/sh
# Renders the icon SVGs to the PNGs the macOS app uses, plus a review sheet. Runs inside a
# Linux container (scripts/docker-test.sh icons) with librsvg and ImageMagick; nothing is
# installed on the host. Output is written into the repository and committed.
set -eu

src=/src/design/icons
assets=/src/apps/macos/Resources/Assets.xcassets
app="$assets/AppIcon.appiconset"
work=/tmp/icons
mkdir -p "$app" "$assets/MenuBarIcon.imageset" "$assets/MenuBarIconPaused.imageset" "$work"

png() { rsvg-convert -w "$2" -h "$2" "$1" -o "$3"; }

for size in 16 32 128 256 512; do
  png "$src/app-icon.svg" "$size" "$app/icon_${size}x${size}.png"
  png "$src/app-icon.svg" $((size * 2)) "$app/icon_${size}x${size}@2x.png"
done
png "$src/menubar.svg" 18 "$assets/MenuBarIcon.imageset/menubar.png"
png "$src/menubar.svg" 36 "$assets/MenuBarIcon.imageset/menubar@2x.png"
png "$src/menubar-paused.svg" 18 "$assets/MenuBarIconPaused.imageset/menubar-paused.png"
png "$src/menubar-paused.svg" 36 "$assets/MenuBarIconPaused.imageset/menubar-paused@2x.png"

# Review sheet: the app icon at 256 and 32, and both menu bar states on light and dark bars,
# magnified 4x with hard edges so the pixels can be judged.
png "$src/app-icon.svg" 256 "$work/app256.png"
png "$src/app-icon.svg" 32 "$work/app32.png"
for name in menubar menubar-paused; do
  png "$src/$name.svg" 36 "$work/$name.png"
  convert "$work/$name.png" -background '#ECECEC' -flatten -filter point -resize 400% "$work/$name-light.png"
  convert "$work/$name.png" -channel RGB -negate +channel -background '#2B2B2B' -flatten -filter point -resize 400% \
    "$work/$name-dark.png"
done
montage "$work/app256.png" "$work/app32.png" "$work/menubar-light.png" "$work/menubar-dark.png" \
  "$work/menubar-paused-light.png" "$work/menubar-paused-dark.png" \
  -tile 6x1 -geometry +16+16 -background '#FFFFFF' -strip "$work/sheet.png"
convert "$work/sheet.png" -strip -define png:exclude-chunks=date,time /src/docs/design/icons-preview.png
for file in "$app"/*.png "$assets"/MenuBarIcon*.imageset/*.png; do
  convert "$file" -strip -define png:exclude-chunks=date,time "$file"
done
echo "icons rendered"
