-- Références croisées dynamiques : quel code lit/écrit une plage mémoire.
-- XREF="debut fin" (hex), XREF_OUT=fichier ; s'utilise avec scenario.lua (deux -autoboot_script ne
-- sont pas possibles : lancer via xref_scenario.lua qui charge les deux).
-- Sortie : une ligne par (pc, r/w, adresse) unique, avec le nombre d'accès.

local cpu = manager.machine.devices[":maincpu"]
local space = cpu.spaces["program"]
local a, b = os.getenv("XREF"):match("^(%x+)%s+(%x+)$")
a, b = tonumber(a, 16), tonumber(b, 16)
local out = os.getenv("XREF_OUT")
local seen = {}

local function pc() return cpu.state["CURPC"].value end
local function note(kind, offset)
	local k = string.format("%06x %s %06x", pc(), kind, offset)
	seen[k] = (seen[k] or 0) + 1
end

xref_r = space:install_read_tap(a, b, "xref_r", function(offset, data, mask) note("r", offset) end)
xref_w = space:install_write_tap(a, b, "xref_w", function(offset, data, mask) note("w", offset) end)

xref_stop = emu.add_machine_stop_notifier(function()
	local keys = {}
	for k in pairs(seen) do keys[#keys + 1] = k end
	table.sort(keys)
	local f = io.open(out, "w")
	for _, k in ipairs(keys) do f:write(string.format("%s %d\n", k, seen[k])) end
	f:close()
	print(string.format("xref: %d entrées -> %s", #keys, out))
end)
