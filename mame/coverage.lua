-- Couverture d'exécution du 68000 CPS2 : enregistre chaque mot lu dans l'espace opcodes
-- (instructions + mots d'extension). Les sessions successives se cumulent dans le même fichier.
--
-- mame ddsom -rompath /home/ycr/som -autoboot_script /home/ycr/som/mame/coverage.lua
-- Sortie : /home/ycr/som/mame/coverage.bin, un octet par mot de ROM (0x200000 octets), 1 = exécuté.

local OUT = "/home/ycr/som/mame/coverage.bin"
local WORDS = 0x200000

local cpu = manager.machine.devices[":maincpu"]
local space = cpu.spaces["opcodes"] or cpu.spaces["program"]

local hit = {}
local f = io.open(OUT, "rb")
if f then
	local data = f:read("a")
	f:close()
	for i = 1, #data do
		if data:byte(i) ~= 0 then hit[i - 1] = true end
	end
end

-- On garde la référence au tap, sinon il est retiré au ramasse-miettes.
tap = space:install_read_tap(0x000000, 0x3fffff, "coverage", function(offset, data, mask)
	hit[offset >> 1] = true
end)

local function save()
	local buf = {}
	for i = 0, WORDS - 1 do buf[#buf + 1] = hit[i] and "\1" or "\0" end
	local o = io.open(OUT, "wb")
	o:write(table.concat(buf))
	o:close()
	local n = 0
	for _ in pairs(hit) do n = n + 1 end
	print(string.format("coverage: %d mots exécutés -> %s", n, OUT))
end

-- Référence globale obligatoire : sinon l'abonnement est collecté et save() n'est jamais appelé.
stop_sub = emu.add_machine_stop_notifier(save)
