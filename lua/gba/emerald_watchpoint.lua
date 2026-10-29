local save_block1 = memory.read_u32(0x03005D8C)
local money_address = save_block1 + 0x490

debug.set_watchpoint(money_address, {pause = false, on = "write", width = "word"})

function debug.on_watchpoint(hit)
    console.log(string.format("%s %s at %08X value=%08X from pc=%08X",
        hit.on, hit.width, hit.address, hit.value, hit.pc))
end
