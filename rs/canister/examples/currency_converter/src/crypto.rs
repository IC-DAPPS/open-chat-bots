use oc_bots_sdk::api::definition::BotCommandOptionChoice;
use std::collections::HashMap;

lazy_static::lazy_static! {
    static ref CRYPTO_CURRENCIES: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        // Top 100 cryptocurrencies by market cap (as of 2023)
        m.insert("BTC", "Bitcoin");
        m.insert("ETH", "Ethereum");
        m.insert("USDT", "Tether");
        m.insert("BNB", "BNB");
        m.insert("XRP", "XRP");
        m.insert("USDC", "USD Coin");
        m.insert("SOL", "Solana");
        m.insert("ADA", "Cardano");
        m.insert("DOGE", "Dogecoin");
        m.insert("TRX", "TRON");
        m.insert("TON", "Toncoin");
        m.insert("DAI", "Dai");
        m.insert("MATIC", "Polygon");
        m.insert("DOT", "Polkadot");
        m.insert("WBTC", "Wrapped Bitcoin");
        m.insert("LTC", "Litecoin");
        m.insert("SHIB", "Shiba Inu");
        m.insert("BCH", "Bitcoin Cash");
        m.insert("AVAX", "Avalanche");
        m.insert("LEO", "UNUS SED LEO");
        m.insert("LINK", "Chainlink");
        m.insert("XLM", "Stellar");
        m.insert("OKB", "OKB");
        m.insert("NEAR", "NEAR Protocol");
        m.insert("APT", "Aptos");
        m.insert("UNI", "Uniswap");
        m.insert("ATOM", "Cosmos");
        m.insert("XMR", "Monero");
        m.insert("ICP", "Internet Computer");
        m.insert("FIL", "Filecoin");
        m.insert("HBAR", "Hedera");
        m.insert("VET", "VeChain");
        m.insert("OP", "Optimism");
        m.insert("MNT", "Mantle");
        m.insert("QNT", "Quant");
        m.insert("RNDR", "Render Token");
        m.insert("ARB", "Arbitrum");
        m.insert("IMX", "Immutable");
        m.insert("CRO", "Cronos");
        m.insert("MKR", "Maker");
        m.insert("GRT", "The Graph");
        m.insert("AAVE", "Aave");
        m.insert("STX", "Stacks");
        m.insert("ALGO", "Algorand");
        m.insert("EOS", "EOS");
        m.insert("EGLD", "MultiversX");
        m.insert("INJ", "Injective");
        m.insert("FTM", "Fantom");
        m.insert("THETA", "Theta Network");
        m.insert("XTZ", "Tezos");
        m.insert("SAND", "The Sandbox");
        m.insert("KAS", "Kaspa");
        m.insert("AXS", "Axie Infinity");
        m.insert("FLOW", "Flow");
        m.insert("MANA", "Decentraland");
        m.insert("NEO", "NEO");
        m.insert("KCS", "KuCoin Token");
        m.insert("BSV", "Bitcoin SV");
        m.insert("CHZ", "Chiliz");
        m.insert("RUNE", "THORChain");
        m.insert("BTT", "BitTorrent");
        m.insert("KAVA", "Kava");
        m.insert("FET", "Fetch.ai");
        m.insert("GALA", "Gala");
        m.insert("HT", "Huobi Token");
        m.insert("MIOTA", "IOTA");
        m.insert("LUNC", "Terra Luna Classic");
        m.insert("XEC", "eCash");
        m.insert("PEPE", "Pepe");
        m.insert("CFX", "Conflux");
        m.insert("IOTA", "IOTA");
        m.insert("GT", "GateToken");
        m.insert("ENJ", "Enjin Coin");
        m.insert("DASH", "Dash");
        m.insert("BGB", "Bitget Token");
        m.insert("XDC", "XDC Network");
        m.insert("ZIL", "Zilliqa");
        m.insert("ZEC", "Zcash");
        m.insert("RVN", "Ravencoin");
        m.insert("1INCH", "1inch Network");
        m.insert("CRV", "Curve DAO Token");
        m.insert("CAKE", "PancakeSwap");
        m.insert("COMP", "Compound");
        m.insert("PAXG", "PAX Gold");
        m.insert("WOO", "WOO Network");
        m.insert("SNX", "Synthetix");
        m.insert("TWT", "Trust Wallet Token");
        m.insert("ENS", "Ethereum Name Service");
        m.insert("BAT", "Basic Attention Token");
        m.insert("GMT", "STEPN");
        m.insert("LRC", "Loopring");
        m.insert("KLAY", "Klaytn");
        m.insert("WAXP", "WAX");
        m.insert("DCR", "Decred");
        m.insert("SC", "Siacoin");
        m.insert("AGIX", "SingularityNET");
        m.insert("WAVES", "Waves");
        m.insert("NEXO", "NEXO");
        m.insert("QTUM", "Qtum");
        m.insert("HOT", "Holo");
        m.insert("KSM", "Kusama");
        m
    };
}

pub fn get_crypto_symbols() -> Vec<String> {
    CRYPTO_CURRENCIES.keys().map(|&k| k.to_string()).collect()
}

pub fn get_crypto_currency_name_from_symbol(symbol: &str) -> Option<&'static str> {
    CRYPTO_CURRENCIES.get(symbol).copied()
}

pub fn get_crypto_currency_choices() -> Vec<BotCommandOptionChoice<String>> {
    CRYPTO_CURRENCIES
        .iter()
        .map(|(&symbol, &name)| {
            let display_name = format!("💰 {} {}", symbol, name);
            
            BotCommandOptionChoice {
                name: display_name,
                value: symbol.to_string(),
            }
        })
        .collect()
}
