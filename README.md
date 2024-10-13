# SoundShift
## Description
This is a program made in rust that changes the volume of selected programs to a specified value when unfocused

## Progress
The program does not work currently. it does about 50% of the intended stuff and is not user friendly.
> 100% would be that it works, aka changes the volume when the specified program **ISN'T** focused and then reverts the volume change when it **IS** focused

## JSON Format
```json
{
  "programs": {
    "Program 1": "Program 1 volume unfocused in decimal number",
    "Program 2": "Program 2 volume unfocused in decimal number",
    ...
  }
}
```
> This will be changed to something that isn't as bad.
> For example the `"programs":` is a bad name.
