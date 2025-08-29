#!/bin/bash

# Solana Universal NFT Deployment Script
# This script handles deployment to different Solana clusters

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
PROGRAM_NAME="solana_universal_nft"
PROGRAM_ID="D9Wnf46z72Wq6g7s4X7KZ8u6y6LmpDrDYKaLi6jHKdZr"

# Default values
CLUSTER="devnet"
UPGRADE=false
VERIFY=false

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -c|--cluster)
            CLUSTER="$2"
            shift 2
            ;;
        -u|--upgrade)
            UPGRADE=true
            shift
            ;;
        -v|--verify)
            VERIFY=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo "Options:"
            echo "  -c, --cluster CLUSTER    Target cluster (localnet|devnet|mainnet-beta)"
            echo "  -u, --upgrade            Upgrade existing program"
            echo "  -v, --verify             Verify deployment after completion"
            echo "  -h, --help               Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option $1"
            exit 1
            ;;
    esac
done

# Validate cluster
case $CLUSTER in
    localnet|devnet|mainnet-beta)
        ;;
    *)
        echo -e "${RED}Error: Invalid cluster '$CLUSTER'. Must be localnet, devnet, or mainnet-beta${NC}"
        exit 1
        ;;
esac

echo -e "${BLUE}🚀 Starting deployment to $CLUSTER...${NC}"

# Set Solana configuration
echo -e "${YELLOW}📝 Setting Solana configuration...${NC}"
solana config set --url $(case $CLUSTER in
    localnet) echo "http://127.0.0.1:8899" ;;
    devnet) echo "https://api.devnet.solana.com" ;;
    mainnet-beta) echo "https://api.mainnet-beta.solana.com" ;;
esac)

# Check wallet balance
echo -e "${YELLOW}💰 Checking wallet balance...${NC}"
BALANCE=$(solana balance --output json | jq -r '.value')
if (( $(echo "$BALANCE < 5" | bc -l) )); then
    echo -e "${RED}Error: Insufficient balance ($BALANCE SOL). Need at least 5 SOL for deployment.${NC}"
    
    if [[ $CLUSTER == "devnet" ]]; then
        echo -e "${YELLOW}💸 Requesting airdrop...${NC}"
        solana airdrop 5
        sleep 5
    else
        exit 1
    fi
fi

# Build the program
echo -e "${YELLOW}🔨 Building program...${NC}"
anchor build

# Deploy or upgrade
if [[ $UPGRADE == true ]]; then
    echo -e "${YELLOW}⬆️  Upgrading program...${NC}"
    anchor upgrade target/deploy/${PROGRAM_NAME}.so --program-id $PROGRAM_ID
else
    echo -e "${YELLOW}🚀 Deploying program...${NC}"
    anchor deploy --program-name $PROGRAM_NAME --program-keypair target/deploy/${PROGRAM_NAME}-keypair.json
fi

# Get deployed program ID
DEPLOYED_PROGRAM_ID=$(solana-keygen pubkey target/deploy/${PROGRAM_NAME}-keypair.json)
echo -e "${GREEN}✅ Program deployed with ID: $DEPLOYED_PROGRAM_ID${NC}"

# Update Anchor.toml with deployed program ID
echo -e "${YELLOW}📝 Updating Anchor.toml...${NC}"
if grep -q "solana_universal_nft" Anchor.toml; then
    sed -i.bak "s/solana_universal_nft = \".*\"/solana_universal_nft = \"$DEPLOYED_PROGRAM_ID\"/" Anchor.toml
else
    echo "solana_universal_nft = \"$DEPLOYED_PROGRAM_ID\"" >> Anchor.toml
fi

# Verify deployment
if [[ $VERIFY == true ]]; then
    echo -e "${YELLOW}🔍 Verifying deployment...${NC}"
    
    # Check if program account exists
    if solana account $DEPLOYED_PROGRAM_ID > /dev/null 2>&1; then
        echo -e "${GREEN}✅ Program account verified${NC}"
        
        # Get program info
        PROGRAM_INFO=$(solana account $DEPLOYED_PROGRAM_ID --output json)
        EXECUTABLE=$(echo $PROGRAM_INFO | jq -r '.account.executable')
        OWNER=$(echo $PROGRAM_INFO | jq -r '.account.owner')
        
        if [[ $EXECUTABLE == "true" && $OWNER == "BPFLoaderUpgradeab1e11111111111111111111111" ]]; then
            echo -e "${GREEN}✅ Program is executable and owned by BPF Loader${NC}"
        else
            echo -e "${RED}❌ Program verification failed${NC}"
            exit 1
        fi
        
        # Test basic functionality
        echo -e "${YELLOW}🧪 Testing basic functionality...${NC}"
        anchor test --skip-build --skip-deploy --skip-local-validator
        
        if [[ $? -eq 0 ]]; then
            echo -e "${GREEN}✅ Basic functionality tests passed${NC}"
        else
            echo -e "${RED}❌ Functionality tests failed${NC}"
            exit 1
        fi
    else
        echo -e "${RED}❌ Program account not found${NC}"
        exit 1
    fi
fi

# Create deployment info file
echo -e "${YELLOW}📄 Creating deployment info...${NC}"
cat > deployment-info.json << EOF
{
    "programId": "$DEPLOYED_PROGRAM_ID",
    "cluster": "$CLUSTER",
    "deploymentTime": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
    "version": "$(git describe --tags --always 2>/dev/null || echo 'unknown')",
    "commit": "$(git rev-parse HEAD 2>/dev/null || echo 'unknown')",
    "rpcUrl": "$(solana config get | grep 'RPC URL' | awk '{print $3}')"
}
EOF

# Initialize the program if it's a fresh deployment
if [[ $UPGRADE == false ]]; then
    echo -e "${YELLOW}🎯 Initializing program...${NC}"
    
    # Create initialize script
    cat > scripts/initialize.js << 'EOF'
const anchor = require('@project-serum/anchor');
const { PublicKey, Keypair } = require('@solana/web3.js');

async function initialize() {
    // Setup
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    
    const program = anchor.workspace.SolanaUniversalNft;
    
    try {
        // Initialize collection
        const [collectionPda] = await PublicKey.findProgramAddress(
            [Buffer.from("nft_collection")],
            program.programId
        );
        
        const tx = await program.methods
            .initialize(
                "Universal NFT Collection",
                "UNC",
                "https://api.example.com/metadata/"
            )
            .accounts({
                authority: provider.wallet.publicKey,
                nftCollection: collectionPda,
                systemProgram: anchor.web3.SystemProgram.programId,
            })
            .rpc();
        
        console.log("✅ Collection initialized with signature:", tx);
        console.log("📦 Collection PDA:", collectionPda.toString());
        
    } catch (error) {
        console.error("❌ Initialization failed:", error);
        process.exit(1);
    }
}

initialize();
EOF
    
    # Run initialization
    if command -v node &> /dev/null && [[ -f package.json ]]; then
        echo -e "${YELLOW}🔧 Running program initialization...${NC}"
        node scripts/initialize.js
    else
        echo -e "${YELLOW}⚠️  Node.js not found or package.json missing. Skipping initialization.${NC}"
        echo -e "${YELLOW}   Run 'anchor run initialize' manually after setting up the JavaScript environment.${NC}"
    fi
fi

# Generate usage examples
echo -e "${YELLOW}📚 Generating usage examples...${NC}"
cat > examples/basic_usage.sh << EOF
#!/bin/bash

# Basic usage examples for Solana Universal NFT
# Make sure to set your cluster and wallet before running

export CLUSTER=$CLUSTER
export PROGRAM_ID=$DEPLOYED_PROGRAM_ID

echo "🔧 Cluster: \$CLUSTER"
echo "📦 Program ID: \$PROGRAM_ID"

# Example 1: Check program account
echo "1️⃣ Checking program account..."
solana account \$PROGRAM_ID

# Example 2: Get collection info (after initialization)
echo "2️⃣ Getting collection info..."
# Add your collection queries here

# Example 3: Mint an NFT
echo "3️⃣ Minting an NFT..."
# Add your minting commands here

echo "✅ Examples completed!"
EOF

chmod +x examples/basic_usage.sh

# Summary
echo -e "${GREEN}"
echo "🎉 Deployment completed successfully!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📦 Program ID: $DEPLOYED_PROGRAM_ID"
echo "🌐 Cluster: $CLUSTER"
echo "💾 Deployment info saved to: deployment-info.json"
echo "📚 Usage examples created in: examples/basic_usage.sh"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "${NC}"

# Next steps
echo -e "${BLUE}📋 Next steps:${NC}"
echo "1. Update your client applications with the new program ID"
echo "2. Test the deployment with: anchor test --skip-build --skip-deploy"
echo "3. Initialize the collection if not done automatically"
echo "4. Deploy to other clusters as needed"

if [[ $CLUSTER == "mainnet-beta" ]]; then
    echo -e "${RED}⚠️  MAINNET DEPLOYMENT COMPLETE${NC}"
    echo -e "${RED}   Double-check all functionality before announcing${NC}"
fi