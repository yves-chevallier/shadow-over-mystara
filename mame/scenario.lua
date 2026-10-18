-- Pilote ddsom image par image : entrées scriptées, captures d'écran et instantanés de la RAM de travail.
-- Configuration par variables d'environnement :
--   SCENARIO  : fichier texte, une action par ligne "image champ valeur" (champ = nom MAME, ex. "P1 Right")
--               ou "image snap" (capture PNG) ou "image quit"
--   RAMDUMP   : "debut fin fichier [pas]" : écrit FF0000-FFFFFF toutes les `pas` images (défaut 1)
--               de [debut, fin] dans fichier
-- mame ddsom -rompath /home/ycr/som -autoboot_script scenario.lua -snapshot_directory snaps ...

local machine = manager.machine
local space = machine.devices[":maincpu"].spaces["program"]

local function find_field(name)
	for _, port in pairs(machine.ioport.ports) do
		local f = port.fields[name]
		if f then return f end
	end
	error("champ d'entrée introuvable : " .. name)
end

local actions = {}
for line in io.lines(os.getenv("SCENARIO")) do
	local frame, rest = line:match("^%s*(%d+)%s+(.+)$")
	if frame then
		local name, value = rest:match("^(.-)%s+(%d+)$")
		local a = { frame = tonumber(frame) }
		local pk, addr, val = rest:match("^poke(%d+)%s+(%x+)%s+(%S+)$")
		if pk then
			-- "poke8|poke16|poke32 adresse valeur" : écriture en RAM de travail (voir scenario.rs)
			a.kind = "poke"; a.size = tonumber(pk); a.addr = tonumber(addr, 16); a.value = tonumber(val)
		elseif rest == "snap" or rest == "quit" then a.kind = rest
		else a.kind = "input"; a.field = find_field(name); a.value = tonumber(value) end
		table.insert(actions, a)
	end
end

local dump_from, dump_to, dump_file, dump_step
local spec = os.getenv("RAMDUMP")
if spec then
	local a, b, f, st = spec:match("^(%d+)%s+(%d+)%s+(%S+)%s*(%d*)$")
	dump_from, dump_to = tonumber(a), tonumber(b)
	dump_step = tonumber(st) or 1
	dump_file = io.open(f, "wb")
end

local frame = 0
frame_sub = emu.add_machine_frame_notifier(function()
	frame = frame + 1
	for _, a in ipairs(actions) do
		if a.frame == frame then
			if a.kind == "input" then a.field:set_value(a.value)
			elseif a.kind == "poke" then
				if a.size == 8 then space:write_u8(a.addr, a.value)
				elseif a.size == 16 then space:write_u16(a.addr, a.value)
				else space:write_u32(a.addr, a.value) end
			elseif a.kind == "snap" then machine.video:snapshot(); print("snap", frame)
			elseif a.kind == "quit" then machine:exit() end
		end
	end
	if dump_file and frame >= dump_from and frame <= dump_to and (frame - dump_from) % dump_step == 0 then
		local parts = {}
		for addr = 0xff0000, 0xfffffe, 2 do
			local w = space:read_u16(addr)
			parts[#parts + 1] = string.char(w >> 8, w & 0xff)
		end
		dump_file:write(table.concat(parts))
	end
end)

stop_sub = emu.add_machine_stop_notifier(function()
	if dump_file then dump_file:close() end
end)
