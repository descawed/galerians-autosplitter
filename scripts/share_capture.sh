#!/usr/bin/env bash

set -euo pipefail

if [[ "$#" -lt 3 ]]
then
  echo Usage: "$0" INPUT_INDEX OBS_INDEX AUTOSPLITTER_INDEX '[--delete-existing]' '[--pixel-format fmt]' '[--resolution WxH]'
  echo '    INPUT_INDEX: index of the video capture device to share'
  echo '    OBS_INDEX: index of the video capture device to create for OBS'
  echo '    AUTOSPLITTER_INDEX: index of the video capture device to create for the autosplitter'
  echo '    --delete-existing: if an output device has already been created, delete it and make a new one'
  echo '    --pixel-format: the pixel format of the input capture; default is yuyv422'
  echo '    --resolution: input resolution in the form WxH; default is 1920x1080'
  exit 1
fi

input_index=$1
obs_index=$2
autosplitter_index=$3

delete_existing=false
pixel_format=yuyv422
resolution=1920x1080

shift 3
while (( "$#" ))
do
  case "$1" in
    --delete-existing)
      delete_existing=true
      ;;
    --pixel-format)
      shift
      pixel_format="$1"
      ;;
    --resolution)
      shift
      resolution="$1"
      ;;
  esac
  shift
done

create_device() {
  local do_create=true
  if v4l2loopback-ctl list 2> /dev/null | grep -wq "$1"
  then
    share_device=$(v4l2loopback-ctl list 2> /dev/null | grep "$1" | cut -f2 | tr -d '[:space:]')
    if [[ "$delete_existing" = true ]]
    then
      echo "Found existing output device $share_device; attempting to delete..."
      v4l2loopback-ctl delete "$share_device"
      echo Device deleted
      share_device="/dev/video$2"
    else
      echo "Output device already exists at $share_device"
      do_create=false
    fi
  else
    share_device="/dev/video$2"
  fi

  if [[ "$do_create" = true ]]
  then
    echo Creating output device...
    v4l2loopback-ctl add -x 1 -n "$1" "$2"
    echo Device created
  fi
}

if ! grep -wq '^v4l2loopback' /proc/modules
then
  echo 'v4l2loopback is not loaded; attempting to load it...'
  modprobe v4l2loopback
  echo v4l2loopback loaded
fi

create_device 'Galerians OBS share' "$obs_index"
obs_device="$share_device"

create_device 'Galerians autosplitter share' "$autosplitter_index"
autosplitter_device="$share_device"

echo Sharing input capture...
ffmpeg -nostdin -y -f v4l2 -input_format "$pixel_format" -video_size "$resolution" -i "/dev/video$input_index" -f v4l2 -pix_fmt "$pixel_format" "$obs_device" -f v4l2 -pix_fmt "$pixel_format" "$autosplitter_device"