local functions = {}

function on_pc(address, fn)
	functions[address] = fn
	debugger.set_breakpoint(address)
end

function debugger.on_breakpoint(address)
	local fn = functions[address]
	if fn then
		fn(address)
	end
end

on_pc(0x080E518C, function()
	print("r0", cpu.read_register("r0"))
end)
