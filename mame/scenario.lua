-- Pilote ddsom image par image : entrées scriptées, captures d'écran et instantanés de la RAM de travail.
-- Configuration par variables d'environnement :
--   SCENARIO  : fichier texte, une action par ligne "image champ valeur" (champ = nom MAME, ex. "P1 Right")
--               ou "image snap" (capture PNG) ou "image quit"
--   RAMDUMP   : "debut fin fichier" : écrit FF0000-FFFFFF à chaque image de [debut, fin] dans fichier
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
		if rest == "snap" or rest == "quit" then a.kind = rest
		else a.kind = "input"; a.field = find_field(name); a.value = tonumber(value) end
		table.insert(actions, a)
	end
end

local dump_from, dump_to, dump_file
local spec = os.getenv("RAMDUMP")
if spec then
	local a, b, f = spec:match("^(%d+)%s+(%d+)%s+(.+)$")
	dump_from, dump_to = tonumber(a), tonumber(b)
	dump_file = io.open(f, "wb")
end

local frame = 0
frame_sub = emu.add_machine_frame_notifier(function()
	frame = frame + 1
	for _, a in ipairs(actions) do
		if a.frame == frame then
			if a.kind == "input" then a.field:set_value(a.value)
			elseif a.kind == "snap" then machine.video:snapshot(); print("snap", frame)
			elseif a.kind == "quit" then machine:exit() end
		end
	end
	if dump_file and frame >= dump_from and frame <= dump_to then
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
