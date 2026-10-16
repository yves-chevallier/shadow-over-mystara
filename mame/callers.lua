-- Appelants dynamiques : à l'entrée de chaque fonction de CALLERS (hex, séparées par des espaces),
-- note le PC courant au moment où l'opcode de la cible est chargé : c'est l'instruction qui y saute
-- (bsr/jsr/jmp/bra). Les fetchs séquentiels depuis l'instruction précédente sont ignorés (PC < cible, proche).
-- CALLERS_OUT=fichier.
local cpu = manager.machine.devices[":maincpu"]
local prog = cpu.spaces["program"]
local ops = cpu.spaces["opcodes"]
local targets, seen = {}, {}
for h in os.getenv("CALLERS"):gmatch("%x+") do targets[tonumber(h, 16)] = true end
callers_taps = {}
for t in pairs(targets) do
	callers_taps[#callers_taps + 1] = ops:install_read_tap(t, t + 1, "callers", function(offset, data, mask)
		local pc = cpu.state["CURPC"].value
		if pc == t or (pc < t and t - pc <= 10) then return end
		local k = string.format("%06x <- %06x", t, pc)
		seen[k] = (seen[k] or 0) + 1
	end)
end
callers_stop = emu.add_machine_stop_notifier(function()
	local f = io.open(os.getenv("CALLERS_OUT"), "w")
	local keys = {}
	for k in pairs(seen) do keys[#keys + 1] = k end
	table.sort(keys)
	for _, k in ipairs(keys) do f:write(k .. " " .. seen[k] .. "\n") end
	f:close()
end)
