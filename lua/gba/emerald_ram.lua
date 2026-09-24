-- https://github.com/pret/pokeemerald/blob/master/src/overworld.c
-- https://github.com/pret/pokeemerald/blob/master/include/global.h
-- https://github.com/pret/pokeemerald/blob/master/src/money.c
-- https://github.com/roytam1/rtoss/blob/master/PokemonHackSourceCode/PokemonMemHack/PokemonMemHackCore.cpp#L947

local save_block1 = 0x03005D8C
local save_block2 = 0x03005D90

local money_pointer = read_u32(save_block1) + 0x490
local encryption_key_pointer = read_u32(save_block2) + 0xAC
local encryption_key = read_u32(encryption_key_pointer)

local current_money = read_u32(money_pointer) ~ encryption_key
print("Current Money:", current_money)

local new_value = encryption_key ~ 999999
write_u32(money_pointer, new_value)

local updated_money = read_u32(money_pointer) ~ encryption_key
print("Updated Money:", updated_money)

local function get_slot_address(slot_index)
    return read_u32(save_block1) + 0x650 + slot_index * 4
end

local slot_address = get_slot_address(7)
local quatity_address = slot_address + 2
local item_key = encryption_key & 0xFFFF

write_u16(slot_address, 1)
write_u16(quatity_address, item_key ~ 99)
print("Item Quantity:", read_u16(quatity_address))
