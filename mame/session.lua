-- Session scriptée complète : scénario + couverture (+ xref et callers si configurés par l'environnement).
local dir = "/home/ycr/som/mame/"
dofile(dir .. "scenario.lua")
dofile(dir .. "coverage.lua")
if os.getenv("XREF") then dofile(dir .. "xref.lua") end
if os.getenv("CALLERS") then dofile(dir .. "callers.lua") end
if os.getenv("VSNAP") then dofile(dir .. "vsnap.lua") end
