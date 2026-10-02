local save_block1 = read_u32(0x03005D8C)
local money_address = save_block1 + 0x490

set_watchpoint(money_address, {pause = false, on = "write", width = "word"})

function on_watchpoint(hit)
    -- repeated info
    print(string.format("%s %s at %08X value=%08X from pc=%08X",
        hit.on, hit.width, hit.address, hit.value, hit.pc))
end
