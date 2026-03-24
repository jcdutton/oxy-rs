# oxy-rs

This is a program written in rust to pull the data files from SpO2 and Heartrate wrist monitors such as the:
1) Viatom Checkme O2 - ViATOM Bluetooth Pulse Oximeter with Alarm,Wrist Blood Oxygen Saturation

Normally one needs to use an Android or iPhone application to read the data files, and then transfer those .dat files to a computer
so that they can be ingested into application such as OSCAR.
https://gitlab.com/CrimsonNape/OSCAR-code

This program can be used to gather various information, using bluetooth, from the Bluetooth Pulse Oximeter, including the .dat files, so that the Android or iPhone app is then not needed.

Useful things to consider when using the Wrist SpO2 meter:
1) It is not at all good at keeping good time, and can loose seconds a day.
It is probably a good idea to run "oxysynctime" just before going to sleep each day, to keeps its time accurate over night. You can use "oxygetinfo" to read the time from the SpO2 meter.
2) In the morning, after a good nights sleep, use the "oxygetfiles" to download the .dat files from the SpO2 meter via bluetooth. It only bothers to copy new files.
3) Then import then new .dat files into OSCAR.



