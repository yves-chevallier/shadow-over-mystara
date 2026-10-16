// Applique symbols.txt : "adresse nom [commentaire]". En ROM, crée ou renomme la fonction ;
// ailleurs, pose un label. Le commentaire devient un commentaire plate.
// Argument : <symbols.txt>
// @category CPS2

import java.io.File;
import java.nio.file.Files;

import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.SourceType;

public class ApplySymbols extends GhidraScript {
	@Override
	protected void run() throws Exception {
		int n = 0;
		for (String line : Files.readAllLines(new File(getScriptArgs()[0]).toPath())) {
			line = line.strip();
			if (line.isEmpty() || line.startsWith("#")) continue;
			String[] p = line.split("\\s+", 3);
			Address a = toAddr(Long.parseLong(p[0], 16));
			String name = p[1];
			boolean rom = a.getOffset() < 0x400000;
			boolean data = name.startsWith("tbl_");
			if (rom && !data) {
				if (getInstructionAt(a) == null) {
					new DisassembleCommand(a, null, true).applyTo(currentProgram, monitor);
				}
				Function f = getFunctionAt(a);
				if (f == null) f = createFunction(a, name);
				if (f != null) f.setName(name, SourceType.USER_DEFINED);
				else createLabel(a, name, true, SourceType.USER_DEFINED);
			}
			else {
				createLabel(a, name, true, SourceType.USER_DEFINED);
			}
			if (p.length > 2) setPlateComment(a, p[2]);
			n++;
		}
		println("symboles appliqués : " + n);
	}
}
