// Exporte le listing (labels, instructions, références) en texte : <sortie.txt>
// @category CPS2
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import java.io.PrintWriter;
public class DumpListing extends GhidraScript {
	@Override protected void run() throws Exception {
		try (PrintWriter w = new PrintWriter(getScriptArgs()[0])) {
			Listing l = currentProgram.getListing();
			for (Instruction i : l.getInstructions(true)) {
				Function f = getFunctionAt(i.getMinAddress());
				if (f != null) w.println("\n; ---- " + f.getName());
				else {
					Symbol s = getSymbolAt(i.getMinAddress());
					if (s != null) w.println(s.getName() + ":");
				}
				w.printf("%06x  %s%n", i.getMinAddress().getOffset(), i);
				if (!i.hasFallthrough()) w.println();
			}
		}
	}
}
