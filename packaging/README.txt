Xbox Series Dashboard @VERSION@
==============================

A console-style home screen for a Windows PC on a TV: your games from Steam, Epic, GOG, EA, Ubisoft, Battle.net
and Xbox, a Guide over your games, played with an Xbox controller. It is the Xbox Series Theme for Playnite, as one
program of its own.


What is in this folder
----------------------

  dashboard.exe               the Dashboard
  WebView2Loader.dll          Microsoft's WebView2 loader, for Settings > Account > Discord (off unless
                              you turn it on). It uses Microsoft Edge WebView2, which Windows 11 has.
  README.txt                  this file
  THIRD-PARTY-LICENSES.txt    the open-source parts it is built from, and their licences
  uninstall.exe               (installed copy only) removes it again

Nothing else comes with it, on purpose:

  - No fonts. It uses Windows' own: Segoe UI, Bahnschrift and Segoe Fluent Icons, which Windows 11 has.
  - No sounds and no boot videos. They are recordings of Microsoft's, and stay where you already have them (below).
  - No store or Discord logos. Their places show plain shapes.


Installed or portable
---------------------

The installer (XboxSeriesDashboard-@VERSION@-setup.exe) needs no administrator. It puts the Dashboard in

  %LOCALAPPDATA%\Programs\XboxSeriesDashboard

adds "Xbox Series Dashboard" to the Start menu, and lists it under Settings > Apps > Installed apps, from where it
uninstalls. Installing a newer version over it keeps everything you set.

The portable zip (XboxSeriesDashboard-@VERSION@-portable.zip) is the same program: unzip it anywhere and run
dashboard.exe. A portable copy can't be the shell (Settings > System > Start with Windows as the shell), since its
folder may move: install it for that.

Either way your settings, Profiles, library and logs are kept in

  %LOCALAPPDATA%\XboxSeriesDashboard

Uninstalling keeps that folder unless you tick "Your settings and library" as you uninstall.


Sounds and boot videos: the media folder
----------------------------------------

The Dashboard plays the console's sounds and a boot video as it starts, if it finds them. It is silent, and goes
straight to Home, without them. It looks in these places, and takes each kind from the first that has any:

  1. The media folder:
       %LOCALAPPDATA%\XboxSeriesDashboard\media\sounds
       %LOCALAPPDATA%\XboxSeriesDashboard\media\videos

  2. The Xbox Series Theme (the Bundle) where Playnite has it installed, under %APPDATA%\Playnite or
     %LOCALAPPDATA%\Playnite. While the Bundle is still installed there, nothing needs copying.

To keep them once Playnite and the Bundle are gone, fill the media folder from the Bundle you have, before you
remove it. In File Explorer, or in a Command Prompt:

  Sounds: copy every .wav file of both of these into %LOCALAPPDATA%\XboxSeriesDashboard\media\sounds

    %APPDATA%\Playnite\Extensions\XboxSeries_eb759ff1-325b-4c34-9a64-d9704f72612b\Sounds
    %APPDATA%\Playnite\Themes\Fullscreen\XboxSeries_Dashboard\audio

  Boot videos: copy every .mp4 file of this into %LOCALAPPDATA%\XboxSeriesDashboard\media\videos

    %APPDATA%\Playnite\Extensions\XboxSeries_eb759ff1-325b-4c34-9a64-d9704f72612b\Videos

  (For a portable Playnite, the same folders are under its own folder.)

  In a Command Prompt:

    mkdir "%LOCALAPPDATA%\XboxSeriesDashboard\media\sounds" "%LOCALAPPDATA%\XboxSeriesDashboard\media\videos"
    copy "%APPDATA%\Playnite\Extensions\XboxSeries_eb759ff1-325b-4c34-9a64-d9704f72612b\Sounds\*.wav" "%LOCALAPPDATA%\XboxSeriesDashboard\media\sounds"
    copy "%APPDATA%\Playnite\Themes\Fullscreen\XboxSeries_Dashboard\audio\*.wav" "%LOCALAPPDATA%\XboxSeriesDashboard\media\sounds"
    copy "%APPDATA%\Playnite\Extensions\XboxSeries_eb759ff1-325b-4c34-9a64-d9704f72612b\Videos\*.mp4" "%LOCALAPPDATA%\XboxSeriesDashboard\media\videos"

Keep the files' names as they are: the Dashboard finds them by name (navigation.wav, XboxSeriesX.mp4 and so on).
Settings > General > Boot video lists the videos it found.


Shell mode
----------

Settings > System > System can start the Dashboard in place of the Windows desktop when you sign in, as a
console starts. The uninstaller turns it off first. To turn it off by hand, from a Command Prompt:

  "%LOCALAPPDATA%\Programs\XboxSeriesDashboard\dashboard.exe" --shell-off


Which version
-------------

Settings > System > About shows the version and the build. From a Command Prompt:

  dashboard.exe --version

This build is not signed, so Windows SmartScreen may ask before it first runs it: choose More info, then Run anyway.
