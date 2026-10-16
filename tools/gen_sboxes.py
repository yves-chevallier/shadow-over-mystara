# Génère cps2dec/src/sboxes.rs à partir de ref/cps2crypt.cpp (MAME, BSD-3-Clause) pour éviter les erreurs de recopie.
import re, sys
src = open(sys.argv[1]).read()
src = re.sub(r'//[^\n]*', '', src)
out = ["// Généré par tools/gen_sboxes.py depuis MAME src/mame/capcom/cps2crypt.cpp (BSD-3-Clause).",
       "use crate::Sbox;", ""]
for fn in (1, 2):
    for r in range(1, 5):
        name = f"fn{fn}_r{r}_boxes"
        m = re.search(r'const sbox ' + name + r'\[4\]\s*=\s*\{(.*?)\};', src, re.S)
        nums = [int(x) for x in re.findall(r'-?\d+', m.group(1))]
        assert len(nums) == 4 * 72, (name, len(nums))
        out.append(f"pub const {name.upper()}: [Sbox; 4] = [")
        for b in range(4):
            n = nums[b*72:(b+1)*72]
            out.append(f"    Sbox {{ table: {n[:64]}, inputs: {n[64:70]}, outputs: {n[70:72]} }},")
        out.append("];")
open(sys.argv[2], "w").write("\n".join(out) + "\n")
