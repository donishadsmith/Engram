-- https://raw.githubusercontent.com/pret/pokeemerald/symbols/pokeemerald.sym
-- https://github.com/pret/pokeemerald/blob/master/include/item.h
-- https://github.com/pret/pokeemerald/blob/master/src/item.c
-- https://github.com/pret/pokeemerald/blob/master/include/constants/items.h
-- https://github.com/pret/pokeemerald/blob/master/src/data/items.h

local function price_offset(item_id)
	return 0x5839A0 + item_id * 44 + 16
end

local function get_price(item_id)
	local low_byte = read_domain("rom", price_offset(item_id))
	local high_byte = read_domain("rom", price_offset(item_id) + 1)

    return (high_byte << 8) | low_byte
end

local function set_price(item_id, price)
    write_domain("rom", price_offset(item_id), price)
    write_domain("rom", price_offset(item_id) + 1, price >> 8)
end

print("Potion Before Cost", get_price(13))

for item_id = 1, 200 do
	set_price(item_id, 1)
end

print("Potion After Cost", get_price(13))
